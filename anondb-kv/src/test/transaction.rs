use super::*;

#[domacro(all_kv_impls)]
fn write_not_visible_until_commit<T: KV>(handle: &T) -> Result<()> {
    let table_name = rand_utf8(10);
    let key = rand::random::<[u8; 32]>();
    let val = rand::random::<[u8; 32]>();

    let write = handle.write_tx()?;

    write.insert(&table_name, key.as_slice(), val.as_slice())?;

    assert!(handle.get(&table_name, key.as_slice())?.is_none());
    assert_eq!(handle.count(&table_name)?, 0);

    write.commit()?;

    assert_eq!(handle.get(&table_name, key.as_slice())?.unwrap(), val);
    assert_eq!(handle.count(&table_name)?, 1);

    Ok(())
}

#[domacro(all_kv_impls)]
fn read_tx_consistent_over_write<T: KV>(handle: &T) -> Result<()> {
    let table_name = rand_utf8(10);
    let key = rand::random::<[u8; 32]>();
    let val = rand::random::<[u8; 32]>();

    handle.insert(&table_name, key.as_slice(), val.as_slice())?;

    let read = handle.read_tx()?;

    assert_eq!(read.count(&table_name)?, 1, "first read is incorrect");

    handle.remove(&table_name, key.as_slice())?;
    assert_eq!(handle.count(&table_name)?, 0);

    assert_eq!(read.count(&table_name)?, 1, "second read is incorrect");

    Ok(())
}

#[domacro(all_kv_impls)]
fn write_not_visible_until_commit_multimap<T: KV>(handle: &T) -> Result<()> {
    let table_name = rand_utf8(10);
    let key = rand::random::<[u8; 32]>();
    let val = rand::random::<[u8; 32]>();

    let write = handle.write_tx()?;

    write.insert_multimap(&table_name, key.as_slice(), val.as_slice())?;

    assert!(
        handle
            .get_multimap(&table_name, key.as_slice())?
            .collect::<Vec<_>>()
            .is_empty()
    );
    assert_eq!(handle.count_multimap(&table_name)?, 0);

    write.commit()?;

    assert_eq!(
        handle
            .get_multimap(&table_name, key.as_slice())?
            .collect::<Result<Vec<_>>>()?
            .first()
            .unwrap()
            .value(),
        val
    );
    assert_eq!(handle.count_multimap(&table_name)?, 1);

    Ok(())
}

#[domacro(all_kv_impls)]
fn read_tx_consistent_over_write_multimap<T: KV>(handle: &T) -> Result<()> {
    let table_name = rand_utf8(10);
    let key = rand::random::<[u8; 32]>();
    let val = rand::random::<[u8; 32]>();

    handle.insert_multimap(&table_name, key.as_slice(), val.as_slice())?;

    let read = handle.read_tx()?;

    assert_eq!(
        read.count_multimap(&table_name)?,
        1,
        "first read is incorrect"
    );

    handle.remove_all_multimap(&table_name, key.as_slice())?;
    assert_eq!(handle.count_multimap(&table_name)?, 0);

    assert_eq!(
        read.count_multimap(&table_name)?,
        1,
        "second read is incorrect"
    );

    Ok(())
}
