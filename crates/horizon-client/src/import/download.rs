//! The phone's first door: game files downloaded from a server, with no computer involved. A phone
//! that opens with nothing to load fetches `<base><version>.zip` (the archive `--pack-resources`
//! writes) into its home, then installs it as an archive that came over Wi-Fi or a cable would be
//! (`Pack`), so what arrives is checked the same way. The download runs on its own thread and the
//! screen reads its progress; if it fails, the other doors are still there.

use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

use super::{Installed, MAX_PACKED, Pack};

/// Where the game files are downloaded from: this followed by `<version>.zip`.
/// `HORIZON_GAME_FILES_URL` replaces it (a folder URL ending in `/`).
pub const BASE_URL: &str = "https://cdn.gayfeet.lol/horizon/";

/// The archive's address for this version.
pub fn url(version: &str) -> String {
    let base = std::env::var("HORIZON_GAME_FILES_URL")
        .ok()
        .filter(|b| !b.is_empty())
        .unwrap_or_else(|| BASE_URL.to_owned());
    let base = if base.ends_with('/') {
        base
    } else {
        format!("{base}/")
    };
    format!("{base}{version}.zip")
}

/// What the download is doing now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stage {
    /// Bytes so far, and the whole size when the server said it (0 when it didn't).
    Downloading { received: u64, total: u64 },
    /// The archive is here and being unpacked.
    Installing,
}

/// A download under way. Dropping it does not stop the thread; the result is simply not read.
pub struct Download {
    received: Arc<AtomicU64>,
    total: Arc<AtomicU64>,
    installing: Arc<AtomicU64>,
    done: mpsc::Receiver<Result<Installed, String>>,
}

impl Download {
    /// Start downloading `url` into `home` and installing it there.
    pub fn start(url: String, home: PathBuf) -> io::Result<Self> {
        let received = Arc::new(AtomicU64::new(0));
        let total = Arc::new(AtomicU64::new(0));
        let installing = Arc::new(AtomicU64::new(0));
        let (sender, done) = mpsc::channel();
        let (r, t, i) = (received.clone(), total.clone(), installing.clone());
        thread::Builder::new()
            .name("horizon-download".into())
            .spawn(move || {
                let result = fetch(&url, &home, &r, &t).and_then(|archive| {
                    i.store(1, Ordering::Relaxed);
                    Pack::open(&archive)
                        .and_then(|pack| pack.install(&home))
                        .map_err(|e| e.to_string())
                });
                let _ = sender.send(result);
            })?;
        Ok(Self {
            received,
            total,
            installing,
            done,
        })
    }

    pub fn stage(&self) -> Stage {
        if self.installing.load(Ordering::Relaxed) != 0 {
            Stage::Installing
        } else {
            Stage::Downloading {
                received: self.received.load(Ordering::Relaxed),
                total: self.total.load(Ordering::Relaxed),
            }
        }
    }

    /// The result, once there is one.
    pub fn poll(&self) -> Option<Result<Installed, String>> {
        match self.done.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Err("the download stopped".into())),
        }
    }
}

/// Download `url` to `home/horizon-download.zip`, through a `.part` file so a cut download is
/// never taken for an archive.
fn fetch(
    url: &str,
    home: &Path,
    received: &AtomicU64,
    total: &AtomicU64,
) -> Result<PathBuf, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(20))
        .timeout_read(Duration::from_secs(60))
        .build();
    let response = agent.get(url).call().map_err(|e| match e {
        ureq::Error::Status(status, _) => format!("the server answered {status} for {url}"),
        other => format!("could not reach {url}: {other}"),
    })?;
    let size = response
        .header("Content-Length")
        .and_then(|l| l.parse::<u64>().ok())
        .unwrap_or(0);
    if size > MAX_PACKED {
        return Err("the archive is larger than this will unpack".into());
    }
    total.store(size, Ordering::Relaxed);
    fs::create_dir_all(home).map_err(|e| e.to_string())?;
    let part = home.join("horizon-download.zip.part");
    let archive = home.join("horizon-download.zip");
    let mut file = File::create(&part).map_err(|e| e.to_string())?;
    let mut reader = response.into_reader();
    let mut buffer = vec![0; 1 << 16];
    let mut so_far = 0u64;
    loop {
        let n = match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => {
                let _ = fs::remove_file(&part);
                return Err(format!("the download was cut: {e}"));
            }
        };
        so_far += n as u64;
        if so_far > MAX_PACKED {
            let _ = fs::remove_file(&part);
            return Err("the archive is larger than this will unpack".into());
        }
        file.write_all(&buffer[..n]).map_err(|e| {
            let _ = fs::remove_file(&part);
            format!("could not save the download: {e}")
        })?;
        received.store(so_far, Ordering::Relaxed);
    }
    if size != 0 && so_far != size {
        let _ = fs::remove_file(&part);
        return Err(format!("the download was cut at {so_far} of {size} bytes"));
    }
    file.sync_all().map_err(|e| e.to_string())?;
    fs::rename(&part, &archive).map_err(|e| e.to_string())?;
    Ok(archive)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_archive_is_named_for_the_version() {
        assert_eq!(url("26.3"), "https://cdn.gayfeet.lol/horizon/26.3.zip");
    }

    /// One HTTP answer from a local socket: `length` in the header, `body` sent.
    fn serve(body: Vec<u8>, length: usize) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 1024];
            let _ = stream.read(&mut request);
            let head =
                format!("HTTP/1.1 200 OK\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n");
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&body);
        });
        format!("http://{address}/26.3.zip")
    }

    fn prepared() -> Vec<u8> {
        use zip::{CompressionMethod, write::SimpleFileOptions, write::ZipWriter};
        let mut zip = ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        for (name, body) in [
            ("horizon-resources.json", r#"{"schema":7}"#),
            ("facts.json", "{}"),
        ] {
            zip.start_file(name, options).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    fn home(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "horizon-download-test-{}-{tag}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn wait(download: &Download) -> Result<Installed, String> {
        for _ in 0..500 {
            if let Some(result) = download.poll() {
                return result;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("the download never finished");
    }

    #[test]
    fn downloaded_game_files_are_installed_where_the_game_reads_them() {
        let body = prepared();
        let size = body.len();
        let home = home("ok");
        let download = Download::start(serve(body, size), home.clone()).unwrap();
        let installed = wait(&download).unwrap();
        assert_eq!(installed.kind, super::super::Kind::Resources);
        assert!(home.join("resources/horizon-resources.json").is_file());
        assert_eq!(download.stage(), Stage::Installing);
        // The archive is consumed, and no partial file is left.
        assert!(!home.join("horizon-download.zip").exists());
        assert!(!home.join("horizon-download.zip.part").exists());
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn a_cut_download_is_refused_and_leaves_nothing() {
        let body = prepared();
        let size = body.len();
        let home = home("cut");
        let download =
            Download::start(serve(body[..size / 2].to_vec(), size), home.clone()).unwrap();
        let error = wait(&download).unwrap_err();
        assert!(error.contains("cut"), "{error}");
        assert!(!home.join("resources").exists());
        assert!(!home.join("horizon-download.zip.part").exists());
        let _ = fs::remove_dir_all(home);
    }
}
