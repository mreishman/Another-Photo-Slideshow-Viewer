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

pub fn folder(path: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files: Vec<_> = fs::read_dir(path)?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .map(|entry| entry.path())
        .filter(|path| {
            matches!(
                path.extension().and_then(|ext| ext.to_str()).map(str::to_ascii_lowercase).as_deref(),
                Some("jpg" | "jpeg" | "png" | "bmp")
            )
        })
        .collect();
    files.sort_by_cached_key(|p| (p.file_name().unwrap_or_default().to_string_lossy().to_lowercase(), p.clone()));
    Ok(files)
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
            while let Ok(mut job) = requests.recv() {
                while let Ok(replacement) = requests.try_recv() {
                    job = replacement;
                }
                let current = || latest.load(Relaxed) == job.generation;
                let send = |output| responses.send((job.generation, output)).is_ok();
                if let Some((index, path)) = job.selected.filter(|_| current()) {
                    let photo = match &cache {
                        Some((cached_path, photo)) if *cached_path == path => Ok(Arc::clone(photo)),
                        _ => decode(&path, (3840, 2160)).map(Arc::new),
                    };
                    if !send(Output::Selected(index, photo)) { return; }
                }
                for (index, path) in job.thumbs {
                    if !current() { break; }
                    if !send(Output::Thumb(index, decode(&path, (256, 144)).ok())) { return; }
                }
                if let Some(path) = job.prefetch.filter(|_| current()) {
                    if let Ok(photo) = decode(&path, (3840, 2160)).map(Arc::new) {
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

