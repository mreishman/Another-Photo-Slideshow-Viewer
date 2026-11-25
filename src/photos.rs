use image::{ImageDecoder, ImageReader, Limits};
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering::Relaxed},
        mpsc::{self, Receiver, Sender},
        Arc,
    },
};

pub struct Photo {
    pub width: i32,
    pub height: i32,
    pub pixels: Vec<u8>,
}

pub fn folder(path: &Path, subfolders: bool) -> io::Result<Vec<PathBuf>> {
    use std::os::windows::fs::MetadataExt;

    let mut files = Vec::new();
    let mut pending = Vec::new();
    let mut entries = fs::read_dir(path)?;
    loop {
        for entry in entries.filter_map(Result::ok) {
            let Ok(kind) = entry.file_type() else { continue; };
            if subfolders && kind.is_dir()
                && entry.metadata().is_ok_and(|meta| meta.file_attributes() & 0x400 == 0)
            {
                pending.push(entry.path());
            } else if kind.is_file() {
                let path = entry.path();
                if supported_path(&path) { files.push(path); }
            }
        }
        let Some(children) = std::iter::from_fn(|| pending.pop()).find_map(|path| fs::read_dir(path).ok()) else { break; };
        entries = children;
    }
    files.sort_by_cached_key(|p| (p.to_string_lossy().to_lowercase(), p.clone()));
    Ok(files)
}

pub fn supported_path(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()).map(str::to_ascii_lowercase).as_deref(),
        Some("jpg" | "jpeg" | "png" | "bmp")
    )
}

fn decode(path: &Path, bounds: (u32, u32)) -> Result<Photo, String> {
    let load = || -> image::ImageResult<Photo> {
        let mut reader = ImageReader::open(path)?.with_guessed_format()?;
        if !matches!(reader.format(), Some(image::ImageFormat::Jpeg | image::ImageFormat::Png | image::ImageFormat::Bmp)) {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Unsupported image format").into());
        }
        let mut limits = Limits::default();
        limits.max_image_width = Some(16_384);
        limits.max_image_height = Some(16_384);
        limits.max_alloc = Some(384 * 1024 * 1024);
        reader.limits(limits.clone());
        let mut decoder = reader.into_decoder()?;
        let (width, height) = decoder.dimensions();
        if u64::from(width) * u64::from(height) > 64_000_000 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Image exceeds 64 megapixels").into());
        }
        decoder.set_limits(limits)?;
        let orientation = decoder.orientation()?;
        let mut image = image::DynamicImage::from_decoder(decoder)?;
        image.apply_orientation(orientation);
        if image.width() > bounds.0 || image.height() > bounds.1 {
            image = image.thumbnail(bounds.0, bounds.1);
        }
        let rgba = image.into_rgba8();
        let (width, height) = rgba.dimensions();
        let mut pixels = rgba.into_raw();
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.swap(0, 2);
            let alpha = u16::from(pixel[3]);
            for (channel, matte) in pixel[..3].iter_mut().zip([20u16, 16, 14]) {
                *channel = ((u16::from(*channel) * alpha + matte * (255 - alpha) + 127) / 255) as u8;
            }
            pixel[3] = 0;
        }
        Ok(Photo { width: width as i32, height: height as i32, pixels })
    };
    load().map_err(|error| error.to_string())
}

pub struct Job {
    pub generation: u64,
    pub folder_epoch: u64,
    pub selected: Option<(usize, PathBuf)>,
    pub thumbs: Vec<(usize, PathBuf)>,
    pub prefetch: Option<PathBuf>,
}

pub enum Output {
    Selected(usize, Result<Arc<Photo>, String>),
    Thumb(usize, Option<Photo>),

}

pub struct Worker {
    jobs: Sender<Job>,
    pub results: Receiver<(u64, Output)>,
    generation: Arc<AtomicU64>,
}

impl Worker {
    pub fn new() -> Self {
        let (jobs, requests) = mpsc::channel::<Job>();
        let (responses, results) = mpsc::channel();
        let generation = Arc::new(AtomicU64::new(0));
        let latest = Arc::clone(&generation);
        std::thread::spawn(move || {
            let mut cache = None;
            let mut folder_epoch = None;
            while let Ok(mut job) = requests.recv() {
                while let Ok(replacement) = requests.try_recv() {
                    job = replacement;
                }
                if folder_epoch != Some(job.folder_epoch) {
                    cache = None;
                    folder_epoch = Some(job.folder_epoch);
                }
                let current = || latest.load(Relaxed) == job.generation;
                let send = |output| responses.send((job.generation, output)).is_ok();
                if let Some((index, path)) = job.selected.filter(|_| current()) {
                    let photo = match &cache {
                        Some((cached_path, photo)) if *cached_path == path => Ok(Arc::clone(photo)),
                        _ => decode(&path, (16_384, 16_384)).map(Arc::new),
                    };
                    if !send(Output::Selected(index, photo)) { return; }
                }
                for (index, path) in job.thumbs {
                    if !current() { break; }
                    if !send(Output::Thumb(index, decode(&path, (256, 144)).ok())) { return; }
                }
                if let Some(path) = job.prefetch.filter(|_| current()) {
                    if let Ok(photo) = decode(&path, (16_384, 16_384)).map(Arc::new) {
                        cache = Some((path, photo));

                    }
                }
            }
        });
        Self { jobs, results, generation }
    }

    pub fn request(&self, job: Job) {
        self.generation.store(job.generation, Relaxed);
        let _ = self.jobs.send(job);
    }
}


#[cfg(test)]
mod tests {
    use super::{decode, folder};
    use image::{Rgb, RgbImage};
    use std::{fs, path::PathBuf, time::SystemTime};

    struct TestFolder(PathBuf);

    impl TestFolder {
        fn new() -> Self {
            let stamp = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "slideshow-viewer-test-{}-{stamp}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestFolder {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn discovers_and_decodes_supported_photos_in_filename_order() {
        let dir = TestFolder::new();
        let pixels = RgbImage::from_pixel(8, 6, Rgb([240, 30, 10]));
        for name in ["03.bmp", "01.jpeg", "02.png"] {
            pixels.save(dir.0.join(name)).unwrap();
        }
        fs::write(dir.0.join("notes.txt"), "ignored").unwrap();
        let nested = dir.0.join("subfolder.png");
        fs::create_dir(&nested).unwrap();
        let nested_path = nested.join("04.png");
        pixels.save(&nested_path).unwrap();
        let recursive = folder(&dir.0, true).unwrap();
        assert_eq!(recursive.len(), 4);
        assert!(recursive.contains(&nested_path));

        let paths = folder(&dir.0, false).unwrap();
        assert_eq!(
            paths,
            ["01.jpeg", "02.png", "03.bmp"].map(|name| dir.0.join(name))
        );
        for path in paths {
            let photo = decode(&path, (4, 4)).unwrap();
            assert_eq!((photo.width, photo.height), (4, 3));
            assert_eq!(photo.pixels.len(), 4 * 3 * 4);
            let bgr = &photo.pixels[..3];
            assert!(bgr[0] < 25 && bgr[1] < 50 && bgr[2] > 220);
        }
    }

    #[test]
    fn reports_corrupt_photo() {
        let dir = TestFolder::new();
        let path = dir.0.join("broken.png");
        fs::write(&path, b"not a PNG").unwrap();
        assert!(decode(&path, (4, 4)).is_err());
    }
}
