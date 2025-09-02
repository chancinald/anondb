use anyhow::Result;
use redb::TableDefinition;

use crate::*;

#[test]
fn open_and_insert() -> Result<()> {
    const TEST_TABLE: TableDefinition<String, String> = TableDefinition::new("test_table");

    let db =
        redb::Database::builder().create_with_backend(redb::backends::InMemoryBackend::new())?;
    let tx = JournaledTransaction::new(db.begin_write()?);
    {
        let mut table = tx.open_table(TEST_TABLE)?;
        table.insert("ahfskasjf".to_string(), "AShaskfasfaf".to_string())?;
    }
    let journal = tx.commit()?;
    assert_eq!(journal.len(), 3);
    Ok(())
}

#[test]
fn open_and_remove() -> Result<()> {
    const TEST_TABLE: TableDefinition<String, String> = TableDefinition::new("test_table");

    let db =
        redb::Database::builder().create_with_backend(redb::backends::InMemoryBackend::new())?;
    let tx = JournaledTransaction::new(db.begin_write()?);
    {
        let mut table = tx.open_table(TEST_TABLE)?;
        table.remove("ahfskasjf".to_string())?;
    }
    let journal = tx.commit()?;
    assert_eq!(journal.len(), 3);
    Ok(())
}
