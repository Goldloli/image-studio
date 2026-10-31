use super::*;
use image::{
    DynamicImage, ExtendedColorType, ImageDecoder, ImageFormat, ImageReader, metadata::Orientation,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::{Cursor, Read, Write};
use studio_core::recipes::SourceFacts;
use studio_core::recipes::{MAX_SOURCE_BYTES, MAX_SOURCE_EDGE, MAX_SOURCE_PIXELS};

const SOURCE_PROCESSING_VERSION: &str = "source-normalize-v1";
const SOURCE_DECODE_LIMIT: u64 = 64 * 1024 * 1024;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedSourceRecord {
    pub asset_id: AssetId,
    pub content_hash: ContentHash,
    pub width: u32,
    pub height: u32,
    pub byte_count: u64,
    pub original_name: String,
    pub original_sha256: String,
    pub orientation: String,
    pub processing_version: String,
    pub has_transparent_pixels: bool,
}

#[derive(Clone, Debug)]
pub struct ResolvedSource {
    pub asset: AssetRef,
    pub width: u32,
    pub height: u32,
    pub byte_count: u64,
    pub has_transparent_pixels: bool,
}

struct PublishedInput {
    path: PathBuf,
    committed: bool,
}

impl Drop for PublishedInput {
    fn drop(&mut self) {
        if !self.committed {
            let quarantine = self
                .path
                .parent()
                .and_then(Path::parent)
                .map(|root| root.join("quarantine"));
            if let Some(directory) = quarantine {
                let target = directory.join(new_key("orphan"));
                let _ = fs::rename(&self.path, target);
            }
        }
    }
}

struct DecodedSource {
    image: DynamicImage,
    orientation: Orientation,
    has_transparent_pixels: bool,
}

impl Library {
    /// Imports one opaque or alpha-bearing still image. `path` comes only from
    /// the host's native file picker and never from a request document.
    pub fn import_source(&mut self, path: &Path) -> Result<ImportedSourceRecord> {
        let mut source = open_regular(path, false)?;
        let metadata = source.metadata()?;
        if metadata.len() == 0 || metadata.len() > MAX_SOURCE_BYTES {
            return Err(LibraryError::Invalid(
                "源图编码文件须为 1 byte 至 32 MiB".into(),
            ));
        }
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        Read::by_ref(&mut source)
            .take(MAX_SOURCE_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.is_empty() || bytes.len() as u64 > MAX_SOURCE_BYTES {
            return Err(LibraryError::Invalid(
                "源图编码文件须为 1 byte 至 32 MiB".into(),
            ));
        }
        let original_hash = digest(&bytes);
        let decoded = decode_source(&bytes)?;
        let width = decoded.image.width();
        let height = decoded.image.height();
        validate_source_dimensions(width, height)?;
        let data_type = if decoded.image.color().has_alpha() {
            AssetKind::ImageRgba
        } else {
            AssetKind::ImageRgb
        };
        let mut normalized = Vec::new();
        decoded
            .image
            .write_to(&mut Cursor::new(&mut normalized), ImageFormat::Png)
            .map_err(|error| LibraryError::Invalid(format!("源图规范化编码失败：{error}")))?;
        if normalized.is_empty() || normalized.len() as u64 > MAX_SOURCE_BYTES {
            return Err(LibraryError::Invalid(
                "规范化 PNG 超过源图 32 MiB 上限".into(),
            ));
        }
        let normalized_hash = digest(&normalized);
        let key = new_key("source");
        let relative_path = format!("inputs/{key}.png");
        let temporary_path = self.root.join("inputs").join(format!("{key}.tmp"));
        let final_path = self.root.join(&relative_path);
        let mut temporary = create_source_file(&temporary_path)?;
        temporary.write_all(&normalized)?;
        temporary.sync_all()?;
        let mut permissions = temporary.metadata()?.permissions();
        permissions.set_readonly(true);
        temporary.set_permissions(permissions)?;
        temporary.sync_all()?;
        drop(temporary);
        fs::rename(&temporary_path, &final_path)?;
        sync_directory(&self.root.join("inputs"))?;
        let mut published = PublishedInput {
            path: final_path,
            committed: false,
        };
        let asset = AssetRef {
            asset_id: AssetId::new(key)?,
            content_hash: normalized_hash.clone(),
            data_type,
            location: StorageLocation::Managed {
                storage_id: StorageLocationId::new("library")?,
                relative_path: relative_path.clone(),
            },
        };
        asset.validate()?;
        let original_name = path
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .filter(|value| !value.is_empty() && value.len() <= 1024)
            .ok_or_else(|| LibraryError::Invalid("源图文件名无效".into()))?;
        let orientation = decoded.orientation.to_exif().to_string();
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO imported_sources(
                id,asset_json,relative_path,width,height,byte_count,original_name,
                original_sha256,orientation,processing_version,has_transparent_pixels,
                created_at
             ) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",
            params![
                asset.asset_id.as_str(),
                serde_json::to_string(&asset)?,
                relative_path,
                width,
                height,
                sqlite_integer(normalized.len() as u64)?,
                original_name,
                original_hash.as_str(),
                orientation,
                SOURCE_PROCESSING_VERSION,
                decoded.has_transparent_pixels,
                now(),
            ],
        )?;
        tx.commit()?;
        published.committed = true;
        Ok(ImportedSourceRecord {
            asset_id: asset.asset_id,
            content_hash: asset.content_hash,
            width,
            height,
            byte_count: normalized.len() as u64,
            original_name,
            original_sha256: original_hash.as_str().into(),
            orientation,
            processing_version: SOURCE_PROCESSING_VERSION.into(),
            has_transparent_pixels: decoded.has_transparent_pixels,
        })
    }

    pub fn imported_sources(&self) -> Result<Vec<ImportedSourceRecord>> {
        let mut statement = self.db.prepare(
            "SELECT id,asset_json,width,height,byte_count,original_name,original_sha256,
                    orientation,processing_version,has_transparent_pixels
             FROM imported_sources ORDER BY created_at DESC,id DESC",
        )?;
        statement
            .query_map([], |row| {
                let asset: AssetRef =
                    serde_json::from_str(&row.get::<_, String>(1)?).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            1,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?;
                Ok(ImportedSourceRecord {
                    asset_id: asset.asset_id.clone(),
                    content_hash: asset.content_hash.clone(),
                    width: row.get(2)?,
                    height: row.get(3)?,
                    byte_count: read_unsigned(row, 4)?,
                    original_name: row.get(5)?,
                    original_sha256: row.get(6)?,
                    orientation: row.get(7)?,
                    processing_version: row.get(8)?,
                    has_transparent_pixels: row.get(9)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn resolve_source(&self, id: &AssetId) -> Result<ResolvedSource> {
        let imported = self
            .db
            .query_row(
                "SELECT asset_json,width,height,byte_count,has_transparent_pixels
                 FROM imported_sources WHERE id=?",
                [id.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        read_unsigned(row, 3)?,
                        row.get(4)?,
                    ))
                },
            )
            .optional()?;
        if let Some((json, width, height, byte_count, has_transparent_pixels)) = imported {
            return Ok(ResolvedSource {
                asset: serde_json::from_str(&json)?,
                width,
                height,
                byte_count,
                has_transparent_pixels,
            });
        }
        let record = self.asset(id)?;
        self.verify_asset(&record)?;
        let bytes = self.read_asset_png(id)?;
        let facts = inspect_normalized_png(&bytes)?;
        Ok(ResolvedSource {
            asset: record.asset,
            width: record.width,
            height: record.height,
            byte_count: record.byte_count,
            has_transparent_pixels: facts,
        })
    }

    pub fn source_facts_for_request(
        &self,
        request: &studio_core::contracts::UserRequest,
    ) -> Result<Option<SourceFacts>> {
        request.validate()?;
        let Some(binding) = request.inputs.first() else {
            return Ok(None);
        };
        let source = self.resolve_source(&binding.asset.asset_id)?;
        if source.asset != binding.asset {
            return Err(LibraryError::Invalid(
                "请求源图身份与本地受控资产不一致".into(),
            ));
        }
        Ok(Some(SourceFacts {
            width: source.width,
            height: source.height,
            has_transparent_pixels: source.has_transparent_pixels,
        }))
    }

    pub fn read_source_png(&self, id: &AssetId) -> Result<Vec<u8>> {
        let source = self.resolve_source(id)?;
        let path = self.source_path(&source.asset)?;
        let bytes = read_limited_regular(&path, MAX_SOURCE_BYTES)?;
        if digest(&bytes) != source.asset.content_hash {
            return Err(LibraryError::Invalid("源图散列不符".into()));
        }
        Ok(bytes)
    }

    pub fn copy_source_snapshot(&self, id: &AssetId, run: &Path) -> Result<()> {
        let bytes = self.read_source_png(id)?;
        let target = run.join("input.png");
        let mut file = create_source_file(&target)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        let mut permissions = file.metadata()?.permissions();
        permissions.set_readonly(true);
        file.set_permissions(permissions)?;
        file.sync_all()?;
        sync_directory(run)?;
        Ok(())
    }

    fn source_path(&self, asset: &AssetRef) -> Result<PathBuf> {
        asset.validate()?;
        let StorageLocation::Managed {
            storage_id,
            relative_path,
        } = &asset.location
        else {
            return Err(LibraryError::Invalid("源图必须属于本地库".into()));
        };
        if storage_id.as_str() != "library" {
            return Err(LibraryError::Invalid("源图存储身份无效".into()));
        }
        if let Some(name) = relative_path
            .strip_prefix("inputs/")
            .and_then(|value| value.strip_suffix(".png"))
        {
            validate_storage_key(name)?;
        } else if relative_path.starts_with("assets/") {
            return self.asset_path(asset);
        } else {
            return Err(LibraryError::Invalid("源图位置无效".into()));
        }
        let path = self.root.join(relative_path);
        if path.parent() != Some(self.root.join("inputs").as_path()) {
            return Err(LibraryError::Invalid("源图路径越界".into()));
        }
        Ok(path)
    }
}

fn validate_source_dimensions(width: u32, height: u32) -> Result<()> {
    if width == 0
        || height == 0
        || width > MAX_SOURCE_EDGE
        || height > MAX_SOURCE_EDGE
        || u64::from(width) * u64::from(height) > MAX_SOURCE_PIXELS
    {
        return Err(LibraryError::Invalid(
            "源图必须为正尺寸、每边不超过 4096 且总像素不超过 4 MP".into(),
        ));
    }
    Ok(())
}

fn decode_source(bytes: &[u8]) -> Result<DecodedSource> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| LibraryError::Invalid(format!("无法识别源图格式：{error}")))?;
    let format = reader
        .format()
        .filter(|format| matches!(format, ImageFormat::Png | ImageFormat::Jpeg))
        .ok_or_else(|| LibraryError::Invalid("源图只接受 PNG 或 JPEG".into()))?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_SOURCE_EDGE);
    limits.max_image_height = Some(MAX_SOURCE_EDGE);
    limits.max_alloc = Some(SOURCE_DECODE_LIMIT);
    reader.limits(limits);
    let mut decoder = reader
        .into_decoder()
        .map_err(|error| LibraryError::Invalid(format!("源图头部无效：{error}")))?;
    let (width, height) = decoder.dimensions();
    validate_source_dimensions(width, height)?;
    if !matches!(
        decoder.original_color_type(),
        ExtendedColorType::Rgb8 | ExtendedColorType::Rgba8
    ) {
        return Err(LibraryError::Invalid(
            "源图只接受 8-bit RGB 或 RGBA；不支持 CMYK、灰度或 16-bit".into(),
        ));
    }
    validate_icc(
        decoder
            .icc_profile()
            .map_err(|error| LibraryError::Invalid(format!("源图颜色配置读取失败：{error}")))?,
    )?;
    if format == ImageFormat::Png {
        validate_input_png(bytes)?;
    }
    let orientation = decoder
        .orientation()
        .map_err(|error| LibraryError::Invalid(format!("源图方向读取失败：{error}")))?;
    let image = DynamicImage::from_decoder(decoder)
        .map_err(|error| LibraryError::Invalid(format!("源图解码失败：{error}")))?;
    let mut image = image;
    image.apply_orientation(orientation);
    validate_source_dimensions(image.width(), image.height())?;
    let has_transparent_pixels = match &image {
        DynamicImage::ImageRgba8(buffer) => buffer.pixels().any(|pixel| pixel[3] != 255),
        DynamicImage::ImageRgb8(_) => false,
        _ => {
            return Err(LibraryError::Invalid(
                "源图解码结果不是 8-bit RGB 或 RGBA".into(),
            ));
        }
    };
    Ok(DecodedSource {
        image,
        orientation,
        has_transparent_pixels,
    })
}

fn validate_icc(profile: Option<Vec<u8>>) -> Result<()> {
    if profile.as_ref().is_some_and(|profile| {
        !profile
            .windows(b"sRGB".len())
            .any(|window| window == b"sRGB")
    }) {
        return Err(LibraryError::Invalid(
            "源图包含暂不支持的 ICC 颜色配置；请导出为 sRGB".into(),
        ));
    }
    Ok(())
}

fn validate_input_png(bytes: &[u8]) -> Result<()> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_limits(png::Limits {
        bytes: SOURCE_DECODE_LIMIT as usize,
    });
    let reader = decoder
        .read_info()
        .map_err(|error| LibraryError::Invalid(format!("PNG 头部无效：{error}")))?;
    let info = reader.info();
    if info.animation_control.is_some() {
        return Err(LibraryError::Invalid("不接受动画 PNG".into()));
    }
    if info.bit_depth != png::BitDepth::Eight
        || !matches!(info.color_type, png::ColorType::Rgb | png::ColorType::Rgba)
    {
        return Err(LibraryError::Invalid("PNG 只接受 8-bit RGB 或 RGBA".into()));
    }
    Ok(())
}

fn inspect_normalized_png(bytes: &[u8]) -> Result<bool> {
    let image = image::load_from_memory_with_format(bytes, ImageFormat::Png)
        .map_err(|error| LibraryError::Invalid(format!("源图校验解码失败：{error}")))?;
    match image {
        DynamicImage::ImageRgba8(buffer) => Ok(buffer.pixels().any(|pixel| pixel[3] != 255)),
        DynamicImage::ImageRgb8(_) => Ok(false),
        _ => Err(LibraryError::Invalid(
            "规范化源图不是 8-bit RGB 或 RGBA".into(),
        )),
    }
}

fn read_limited_regular(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let file = open_regular(path, false)?;
    if file.metadata()?.len() > limit {
        return Err(LibraryError::Invalid("源图超过 32 MiB".into()));
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(LibraryError::Invalid("源图超过 32 MiB".into()));
    }
    Ok(bytes)
}

fn create_source_file(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    options.open(path).map_err(Into::into)
}

fn digest(bytes: &[u8]) -> ContentHash {
    ContentHash::new(format!("{:x}", Sha256::digest(bytes))).expect("SHA-256 output")
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GenericImageView, ImageBuffer, Luma, Rgb};

    fn open_library(root: &Path) -> Library {
        Library::open(root, vec![]).unwrap()
    }

    #[test]
    fn imports_an_immutable_normalized_source_and_ignores_the_original() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let original = outside.path().join("photo.png");
        let image = ImageBuffer::from_fn(3, 2, |x, y| Rgb([x as u8, y as u8, 7u8]));
        image.save(&original).unwrap();
        let mut library = open_library(root.path());
        let imported = library.import_source(&original).unwrap();
        fs::remove_file(original).unwrap();
        let bytes = library.read_source_png(&imported.asset_id).unwrap();
        assert_eq!(
            image::load_from_memory(&bytes).unwrap().dimensions(),
            (3, 2)
        );
        assert_eq!(imported.width, 3);
        assert_eq!(imported.height, 2);
        assert!(!imported.has_transparent_pixels);
        assert_eq!(library.imported_sources().unwrap().len(), 1);
        assert_eq!(library.assets(100, 0).unwrap().len(), 0);
    }

    #[test]
    fn rgba_source_retains_alpha_and_reports_transparency() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let original = outside.path().join("alpha.png");
        let image = ImageBuffer::from_fn(2, 2, |x, _| {
            image::Rgba([10u8, 20, 30, if x == 0 { 0 } else { 255 }])
        });
        image.save(&original).unwrap();
        let mut library = open_library(root.path());
        let imported = library.import_source(&original).unwrap();
        assert!(imported.has_transparent_pixels);
        let resolved = library.resolve_source(&imported.asset_id).unwrap();
        assert_eq!(resolved.asset.data_type, AssetKind::ImageRgba);
        assert_eq!(resolved.width, 2);
        assert_eq!(resolved.height, 2);
    }

    #[test]
    fn jpeg_exif_orientation_is_applied_once() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let original = outside.path().join("oriented.jpg");
        ImageBuffer::from_fn(3, 2, |x, y| Rgb([x as u8, y as u8, 40u8]))
            .save(&original)
            .unwrap();
        let jpeg = fs::read(&original).unwrap();
        let mut exif =
            b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0".to_vec();
        let segment_length = (exif.len() + 2) as u16;
        let mut with_exif = Vec::with_capacity(jpeg.len() + exif.len() + 4);
        with_exif.extend_from_slice(&jpeg[..2]);
        with_exif.extend_from_slice(&[0xff, 0xe1]);
        with_exif.extend_from_slice(&segment_length.to_be_bytes());
        with_exif.append(&mut exif);
        with_exif.extend_from_slice(&jpeg[2..]);
        fs::write(&original, with_exif).unwrap();

        let mut library = open_library(root.path());
        let imported = library.import_source(&original).unwrap();
        assert_eq!((imported.width, imported.height), (2, 3));
        assert_eq!(imported.orientation, "6");
        let normalized = library.read_source_png(&imported.asset_id).unwrap();
        assert_eq!(
            image::load_from_memory(&normalized).unwrap().dimensions(),
            (2, 3)
        );
    }

    #[test]
    fn rejects_grayscale_fake_and_oversize_inputs_before_storage() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let mut library = open_library(root.path());
        let grayscale = outside.path().join("gray.png");
        ImageBuffer::from_pixel(2, 2, Luma([128u8]))
            .save(&grayscale)
            .unwrap();
        assert!(
            library
                .import_source(&grayscale)
                .unwrap_err()
                .to_string()
                .contains("RGB")
        );
        let fake = outside.path().join("fake.png");
        fs::write(&fake, b"not an image").unwrap();
        assert!(library.import_source(&fake).is_err());
        let oversized = outside.path().join("oversized.bin");
        fs::write(&oversized, vec![0u8; MAX_SOURCE_BYTES as usize + 1]).unwrap();
        assert!(library.import_source(&oversized).is_err());
        assert!(library.imported_sources().unwrap().is_empty());
        assert_eq!(fs::read_dir(root.path().join("inputs")).unwrap().count(), 0);
    }
}
