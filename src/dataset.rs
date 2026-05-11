use std::future::Future;
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::pin::Pin;

use indicatif::ProgressBar;

use crate::error::{MascotError, Result};

/// Boxed future returned by dataset retrieval trait methods.
pub type DatasetFuture<T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'static>>;

#[derive(Debug, Clone)]
#[cfg_attr(feature = "mem_size", derive(mem_dbg::MemSize))]
#[cfg_attr(feature = "mem_dbg", derive(mem_dbg::MemDbg))]
pub(crate) struct SingleFileDatasetConfig {
    url: String,
    target_directory: PathBuf,
    file_name: String,
    verbose: bool,
    force_download: bool,
    progress_message: &'static str,
}

impl SingleFileDatasetConfig {
    pub(crate) fn new(
        url: &str,
        target_directory: PathBuf,
        file_name: &str,
        progress_message: &'static str,
    ) -> Self {
        Self {
            url: url.to_string(),
            target_directory,
            file_name: file_name.to_string(),
            verbose: false,
            force_download: false,
            progress_message,
        }
    }

    pub(crate) fn set_url<S: Into<String>>(&mut self, url: S) {
        self.url = url.into();
    }

    pub(crate) fn set_target_directory<PathLike: AsRef<Path>>(
        &mut self,
        target_directory: PathLike,
    ) {
        self.target_directory = target_directory.as_ref().to_path_buf();
    }

    pub(crate) fn set_file_name<S: Into<String>>(&mut self, file_name: S) {
        self.file_name = file_name.into();
    }

    pub(crate) const fn enable_verbose(&mut self) {
        self.verbose = true;
    }

    pub(crate) const fn set_force_download(&mut self, force_download: bool) {
        self.force_download = force_download;
    }

    pub(crate) fn path(&self) -> PathBuf {
        self.target_directory.join(&self.file_name)
    }

    pub(crate) fn download(&self) -> Result<SingleFileDatasetDownload> {
        if self.file_name.is_empty() {
            return Err(MascotError::EmptyFilename);
        }

        let path = self.path();
        std::fs::create_dir_all(&self.target_directory).map_err(|source| MascotError::Io {
            path: self.target_directory.display().to_string(),
            source,
        })?;

        let bytes = if !self.force_download
            && path.try_exists().map_err(|source| MascotError::Io {
                path: path.display().to_string(),
                source,
            })? {
            std::fs::metadata(&path)
                .map_err(|source| MascotError::Io {
                    path: path.display().to_string(),
                    source,
                })?
                .len()
        } else {
            self.download_to_path(&path)?
        };

        Ok(SingleFileDatasetDownload { path, bytes })
    }

    fn download_to_path(&self, path: &Path) -> Result<u64> {
        let mut response = ureq::get(&self.url)
            .call()
            .map_err(|source| MascotError::Download {
                url: self.url.clone(),
                source: Box::new(source),
            })?;
        let progress_bar = self.progress_bar(Self::content_length(&response));
        let file = std::fs::File::create(path).map_err(|source| MascotError::Io {
            path: path.display().to_string(),
            source,
        })?;
        let mut writer = BufWriter::new(file);
        let mut reader = response.body_mut().as_reader();
        let mut buffer = vec![0_u8; 1024 * 1024].into_boxed_slice();
        let mut downloaded_bytes = 0_u64;

        loop {
            let read_bytes = reader.read(&mut buffer).map_err(|source| MascotError::Io {
                path: path.display().to_string(),
                source,
            })?;
            if read_bytes == 0 {
                break;
            }
            writer
                .write_all(&buffer[..read_bytes])
                .map_err(|source| MascotError::Io {
                    path: path.display().to_string(),
                    source,
                })?;
            let read_bytes = u64::try_from(read_bytes).map_err(|_| MascotError::Io {
                path: path.display().to_string(),
                source: std::io::Error::other("download chunk length does not fit in u64"),
            })?;
            downloaded_bytes += read_bytes;
            if let Some(progress_bar) = &progress_bar {
                progress_bar.inc(read_bytes);
            }
        }

        writer.flush().map_err(|source| MascotError::Io {
            path: path.display().to_string(),
            source,
        })?;
        if let Some(progress_bar) = progress_bar {
            progress_bar.finish_and_clear();
        }

        Ok(downloaded_bytes)
    }

    fn progress_bar(&self, content_length: Option<u64>) -> Option<ProgressBar> {
        if !self.verbose {
            return None;
        }

        let progress_bar = content_length.map_or_else(ProgressBar::new_spinner, ProgressBar::new);
        progress_bar.set_message(self.progress_message);
        Some(progress_bar)
    }

    fn content_length(response: &ureq::http::Response<ureq::Body>) -> Option<u64> {
        response
            .headers()
            .get("content-length")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "mem_size", derive(mem_dbg::MemSize))]
#[cfg_attr(feature = "mem_dbg", derive(mem_dbg::MemDbg))]
pub(crate) struct SingleFileDatasetDownload {
    path: PathBuf,
    bytes: u64,
}

impl SingleFileDatasetDownload {
    pub(crate) fn into_parts(self) -> (PathBuf, u64) {
        (self.path, self.bytes)
    }
}

/// Common interface for downloadable datasets exposed by this crate.
///
/// Implementations provide a `download` step that only ensures local files are
/// present and a `load` step that downloads if needed and parses the dataset.
pub trait Dataset {
    /// Result returned after the dataset files are present locally.
    type Download;

    /// Iterator returned after the dataset files are present locally.
    type Iter: Iterator;

    /// Result returned after the local dataset files are parsed.
    type Load;

    /// Downloads or reuses the local dataset files without parsing them.
    fn download(self) -> DatasetFuture<Self::Download>;

    /// Downloads or reuses the local dataset files and returns a streaming MGF iterator.
    fn mgf_iter(self) -> DatasetFuture<Self::Iter>;

    /// Downloads the dataset if needed and parses it.
    fn load(self) -> DatasetFuture<Self::Load>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_temp_directory(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("{name}-{}", std::process::id()))
    }

    #[test]
    fn progress_bar_respects_verbose_flag() {
        let quiet = SingleFileDatasetConfig::new(
            "https://example.invalid/file.mgf",
            std::env::temp_dir(),
            "file.mgf",
            "Downloading test file",
        );
        assert!(quiet.progress_bar(Some(10)).is_none());

        let mut verbose = quiet;
        verbose.enable_verbose();
        let progress_bar = verbose.progress_bar(None);
        assert!(progress_bar.is_some());
        if let Some(progress_bar) = progress_bar {
            progress_bar.finish_and_clear();
        }
    }

    #[test]
    fn parses_content_length_header() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let response = ureq::http::Response::builder()
            .header("content-length", "42")
            .body(ureq::Body::builder().data(Vec::new()))?;
        assert_eq!(SingleFileDatasetConfig::content_length(&response), Some(42));

        let response = ureq::http::Response::builder()
            .header("content-length", "not-a-number")
            .body(ureq::Body::builder().data(Vec::new()))?;
        assert_eq!(SingleFileDatasetConfig::content_length(&response), None);

        let response =
            ureq::http::Response::builder().body(ureq::Body::builder().data(Vec::new()))?;
        assert_eq!(SingleFileDatasetConfig::content_length(&response), None);

        Ok(())
    }

    #[test]
    fn download_reuses_cached_file() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let target_directory = unique_temp_directory("mascot-rs-single-file-cache-test");
        let _ = std::fs::remove_dir_all(&target_directory);
        std::fs::create_dir_all(&target_directory)?;
        let config = SingleFileDatasetConfig::new(
            "https://example.invalid/file.mgf",
            target_directory.clone(),
            "cached.mgf",
            "Downloading cached file",
        );
        let path = config.path();
        let contents = b"cached contents\n";
        std::fs::write(&path, contents)?;

        let download = config.download()?;
        let (downloaded_path, downloaded_bytes) = download.into_parts();
        std::fs::remove_dir_all(&target_directory)?;

        assert_eq!(downloaded_path, path);
        assert_eq!(downloaded_bytes, contents.len() as u64);

        Ok(())
    }

    #[test]
    fn download_reports_target_directory_errors(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let target_directory = unique_temp_directory("mascot-rs-single-file-directory-error-test");
        let _ = std::fs::remove_dir_all(&target_directory);
        std::fs::write(&target_directory, b"not a directory")?;
        let config = SingleFileDatasetConfig::new(
            "https://example.invalid/file.mgf",
            target_directory.clone(),
            "cached.mgf",
            "Downloading cached file",
        );

        let result = config.download();
        std::fs::remove_file(&target_directory)?;

        assert!(matches!(result, Err(MascotError::Io { .. })));

        Ok(())
    }

    #[test]
    fn download_reports_http_errors() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0))?;
        let url = format!("http://{}/file.mgf", listener.local_addr()?);
        drop(listener);
        let target_directory = unique_temp_directory("mascot-rs-single-file-download-error-test");
        let _ = std::fs::remove_dir_all(&target_directory);
        let mut config = SingleFileDatasetConfig::new(
            &url,
            target_directory,
            "downloaded.mgf",
            "Downloading test file",
        );
        config.set_force_download(true);

        assert!(matches!(
            config.download(),
            Err(MascotError::Download { .. })
        ));
        let _ = std::fs::remove_dir_all(config.target_directory);

        Ok(())
    }

    #[test]
    fn download_fetches_file_from_http_server(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let target_directory = unique_temp_directory("mascot-rs-single-file-http-test");
        let _ = std::fs::remove_dir_all(&target_directory);
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0))?;
        let url = format!("http://{}/file.mgf", listener.local_addr()?);
        let body = b"downloaded contents\n";
        let server = std::thread::spawn(move || -> std::io::Result<()> {
            let (mut stream, _) = listener.accept()?;
            stream.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
            let mut request_buffer = [0_u8; 1024];
            let _ = stream.read(&mut request_buffer)?;
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(header.as_bytes())?;
            stream.write_all(body)?;
            stream.flush()
        });

        let mut config = SingleFileDatasetConfig::new(
            &url,
            target_directory.clone(),
            "downloaded.mgf",
            "Downloading test file",
        );
        config.enable_verbose();
        let download = config.download()?;
        server
            .join()
            .map_err(|_| std::io::Error::other("HTTP server thread panicked"))??;
        let (downloaded_path, downloaded_bytes) = download.into_parts();
        let downloaded_contents = std::fs::read(&downloaded_path)?;
        std::fs::remove_dir_all(&target_directory)?;

        assert_eq!(downloaded_bytes, body.len() as u64);
        assert_eq!(downloaded_contents, body);

        Ok(())
    }
}
