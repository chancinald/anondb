use anyhow::Result;

use crate::*;

#[test]
fn open_and_insert() -> Result<()> {
    let db = Journal::in_memory(None)?;
    db.insert("test2", &"test_key".to_string(), &"test_value".to_string())?;
    assert_eq!(db.get_state()?.next_tx_index, 1);
    db.insert("test3", &"test_key".to_string(), &"test_value".to_string())?;
    assert_eq!(db.get_state()?.next_tx_index, 2);
    Ok(())
}

#[test]
fn open_and_tx() -> Result<()> {
    let db = Journal::in_memory(None)?;
    db.insert("test2", &"test_key".to_string(), &"test_value".to_string())?;
    let mut tx = db.begin_write()?;
    {
        let mut table = tx.open_table("test")?;
        table.insert(&"ahfskasjf".to_string(), &"AShaskfasfaf".to_string())?;
    }
    let operations = tx.commit()?;
    assert_eq!(operations.len(), 3);
    Ok(())
}

#[test]
fn open_and_get() -> Result<()> {
    let db = Journal::in_memory(None)?;
    let mut tx = db.begin_write()?;
    let test_val = "tsdfadasdgasgag".to_string();
    {
        let mut table = tx.open_table("test")?;
        table.insert(&"test_key".to_string(), &test_val)?;
    }
    tx.commit()?;

    assert_eq!(
        db.get::<String, String>("test", &"test_key".to_string())?
            .expect("get operation errored in open_and_get"),
        test_val
    );

    Ok(())
}

#[test]
fn open_and_remove() -> Result<()> {
    let db = Journal::in_memory(None)?;
    let mut tx = db.begin_write()?;
    {
        let mut table = tx.open_table("test")?;
        table.remove(&"ahfskasjf".to_string())?;
    }
    let operations = tx.commit()?;
    assert_eq!(operations.len(), 3);
    Ok(())
}
