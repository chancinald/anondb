use anyhow::Result;

use crate::*;

#[test]
fn open_and_find_one() -> Result<()> {
    let db = Journal::in_memory(None)?;
    db.insert("table", &"key1".to_string(), &"test_value_1".to_string())?;
    db.insert("table", &"key2".to_string(), &"test_value_2".to_string())?;

    let record = db
        .find_one::<String, String, _>("table", |key, value| {
            if value.ends_with("2") {
                Some((key, value))
            } else {
                None
            }
        })?
        .expect("did not find record");

    assert_eq!(record.0, "key2");
    assert_eq!(record.1, "test_value_2");

    Ok(())
}

#[test]
fn open_and_find_many() -> Result<()> {
    let db = Journal::in_memory(None)?;
    db.insert("table", &"key1".to_string(), &"test_value_1".to_string())?;
    db.insert("table", &"key2".to_string(), &"test_value_2".to_string())?;

    let records = db.find_many::<String, String, _>("table", |_key, _value| true)?;

    assert_eq!(records.len(), 2);

    Ok(())
}

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
    // first transaction
    db.insert("test2", &"test_key".to_string(), &"test_value".to_string())?;
    // second transaction
    let mut tx = db.begin_write()?;
    {
        let mut table = tx.open_table("test")?;
        table.insert(&"ahfskasjf".to_string(), &"AShaskfasfaf".to_string())?;
    }
    tx.commit()?;

    let transactions = db.drain_transactions()?;
    assert_eq!(transactions.len(), 2);
    assert_eq!(transactions.get(0).unwrap().operations.len(), 3);
    assert_eq!(transactions.get(1).unwrap().operations.len(), 3);
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
    tx.commit()?;

    let transactions = db.drain_transactions()?;
    assert_eq!(transactions.len(), 1);
    assert_eq!(transactions.first().unwrap().operations.len(), 3);
    Ok(())
}
