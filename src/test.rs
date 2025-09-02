use redb::TableDefinition;

use crate::*;

#[test]
fn open_and_insert() -> anyhow::Result<()> {
    const TEST_TABLE: TableDefinition<DynValue, DynValue> = TableDefinition::new("test_table");

    let db =
        redb::Database::builder().create_with_backend(redb::backends::InMemoryBackend::new())?;
    let tx = JournaledTransaction::<DynValue>::new(db.begin_write()?);
    {
        let mut table = tx.open_table(TEST_TABLE)?;
        table.insert(
            DynValue::String("ahfskasjf".into()),
            DynValue::String("AShaskfasfaf".into()),
        )?;
    }
    let journal = tx.commit()?;
    assert_eq!(journal.len(), 2);
    Ok(())
}
