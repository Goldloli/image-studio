use super::*;

pub(super) const CURRENT_USER_VERSION: u32 = 5;

/// Migrates the first-generation single-output schema without rewriting IDs,
/// paths, requests, plans, manifests, or image bytes.
pub(super) fn migrate_v4_to_v5(db: &mut Connection) -> Result<()> {
    let version: u32 = db.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version == CURRENT_USER_VERSION {
        return Ok(());
    }
    if version != 4 {
        return Err(LibraryError::Schema(format!(
            "v4 migration cannot upgrade user_version={version}"
        )));
    }
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.execute_batch(
        "
        ALTER TABLE generation_turns
            ADD COLUMN capability_id TEXT NOT NULL
            DEFAULT 'image.text-to-image.z-image-turbo.v1';
        ALTER TABLE generation_turns ADD COLUMN source_asset_id TEXT;

        CREATE TABLE imported_sources(
            id TEXT PRIMARY KEY,
            asset_json TEXT NOT NULL,
            relative_path TEXT NOT NULL UNIQUE,
            width INTEGER NOT NULL CHECK(width > 0),
            height INTEGER NOT NULL CHECK(height > 0),
            byte_count INTEGER NOT NULL CHECK(byte_count > 0),
            original_name TEXT NOT NULL,
            original_sha256 TEXT NOT NULL,
            orientation TEXT NOT NULL,
            processing_version TEXT NOT NULL,
            has_transparent_pixels INTEGER NOT NULL DEFAULT 0
                CHECK(has_transparent_pixels IN (0,1)),
            created_at INTEGER NOT NULL
        );
        CREATE INDEX imported_sources_created
            ON imported_sources(created_at DESC, id DESC);

        ALTER TABLE assets RENAME TO assets_v4;
        CREATE TABLE assets(
            id TEXT PRIMARY KEY,
            job_id TEXT NOT NULL REFERENCES jobs(id),
            attempt_id TEXT NOT NULL REFERENCES attempts(id),
            output_role TEXT NOT NULL DEFAULT 'primary'
                CHECK(output_role IN ('primary','mask')),
            asset_json TEXT NOT NULL,
            relative_path TEXT NOT NULL UNIQUE,
            width INTEGER NOT NULL,
            height INTEGER NOT NULL,
            byte_count INTEGER NOT NULL,
            created_at INTEGER NOT NULL,
            UNIQUE(attempt_id, output_role)
        );
        INSERT INTO assets(
            id,job_id,attempt_id,output_role,asset_json,relative_path,
            width,height,byte_count,created_at
        )
        SELECT
            id,job_id,attempt_id,'primary',asset_json,relative_path,
            width,height,byte_count,created_at
        FROM assets_v4;
        DROP TABLE assets_v4;
        CREATE INDEX assets_created ON assets(created_at DESC, id DESC);
        CREATE INDEX assets_attempt_role ON assets(attempt_id, output_role);
        PRAGMA user_version=5;
        ",
    )?;
    let violations: u32 =
        tx.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })?;
    if violations != 0 {
        return Err(LibraryError::Schema(format!(
            "v4 to v5 migration found {violations} foreign-key violations"
        )));
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_is_rejected_for_an_unexpected_intermediate_version() {
        let mut db = Connection::open_in_memory().unwrap();
        db.pragma_update(None, "user_version", 3).unwrap();
        assert!(migrate_v4_to_v5(&mut db).is_err());
    }
}
