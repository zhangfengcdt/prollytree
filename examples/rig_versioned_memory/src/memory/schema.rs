use anyhow::Result;
use gluesql_core::prelude::Glue;
use prollytree::sql::ProllyStorage;

pub async fn setup_schema(glue: &mut Glue<ProllyStorage<32>>) -> Result<()> {
    // Create short-term memory table
    glue.execute(
        r#"
        CREATE TABLE IF NOT EXISTS short_term_memory (
            id TEXT PRIMARY KEY,
            session_id TEXT,
            timestamp INTEGER,
            role TEXT,
            content TEXT,
            metadata TEXT
        )
    "#,
    )
    .await?;

    // Create long-term memory table
    glue.execute(
        r#"
        CREATE TABLE IF NOT EXISTS long_term_memory (
            id TEXT PRIMARY KEY,
            concept TEXT,
            facts TEXT,
            confidence FLOAT,
            created_at INTEGER,
            access_count INTEGER
        )
    "#,
    )
    .await?;

    // Create episodic memory table
    glue.execute(
        r#"
        CREATE TABLE IF NOT EXISTS episodic_memory (
            id TEXT PRIMARY KEY,
            episode_id TEXT,
            timestamp INTEGER,
            context TEXT,
            action_taken TEXT,
            outcome TEXT,
            reward FLOAT
        )
    "#,
    )
    .await?;

    // Create memory associations table
    glue.execute(
        r#"
        CREATE TABLE IF NOT EXISTS memory_links (
            source_type TEXT,
            source_id TEXT,
            target_type TEXT,
            target_id TEXT,
            relation_type TEXT,
            strength FLOAT
        )
    "#,
    )
    .await?;

    // Note: Indexes are not supported by ProllyStorage yet
    // Future enhancement: implement index support in ProllyStorage

    Ok(())
}
