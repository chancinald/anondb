use super::*;

#[domacro(all_kv_impls)]
fn range_equal_len<T: KV>(handle: &T) -> Result<()> {
    const LEN: usize = 100;
    const MIDPOINT: usize = LEN / 2;

    let table_name = rand_utf8(10);
    let mut entries = Vec::<[u8; 32]>::default();
    for _ in 0..LEN {
        let key = rand::random();
        entries.push(key);
        handle.insert(&table_name, key.as_slice(), &[0; 32])?;
    }
    entries.sort();

    let all_entries = handle.range(&table_name, ..)?.collect::<Result<Vec<_>>>()?;
    assert_eq!(all_entries.len(), entries.len());
    for (v0, v1) in all_entries.iter().zip(entries.iter()) {
        assert_eq!(v0.key(), v1);
    }

    let midpoint = entries.get(MIDPOINT).unwrap();
    {
        let upper_entries = handle
            .range(&table_name, midpoint.as_slice()..)?
            .collect::<Result<Vec<_>>>()?;
        assert_eq!(upper_entries.len(), entries[MIDPOINT..].len());
        for (v0, v1) in upper_entries.iter().zip(entries[MIDPOINT..].iter()) {
            assert_eq!(v0.key(), v1);
        }
    }
    {
        let lower_entries = handle
            .range(&table_name, ..midpoint.as_slice())?
            .collect::<Result<Vec<_>>>()?;
        assert_eq!(lower_entries.len(), entries[..MIDPOINT].len());
        for (v0, v1) in lower_entries.iter().zip(entries[..MIDPOINT].iter()) {
            assert_eq!(v0.key(), v1);
        }
    }
    {
        let midpoint = entries.get(50).unwrap();
        let lower_inclusive_entries = handle
            .range(&table_name, ..=midpoint.as_slice())?
            .collect::<Result<Vec<_>>>()?;
        assert_eq!(lower_inclusive_entries.len(), entries[..=MIDPOINT].len());
        for (v0, v1) in lower_inclusive_entries
            .iter()
            .zip(entries[..=MIDPOINT].iter())
        {
            assert_eq!(v0.key(), v1);
        }
    }

    Ok(())
}

#[domacro(all_kv_impls)]
fn range_var_len<T: KV>(handle: &T) -> Result<()> {
    const LEN: usize = 100;
    const MIDPOINT: usize = LEN / 2;

    let table_name = rand_utf8(10);
    let mut entries = Vec::<Vec<u8>>::default();
    for _ in 0..LEN {
        let byte_len = rand::random::<u16>();
        let mut key = Vec::<u8>::default();
        for _ in 0..byte_len {
            key.push(rand::random());
        }
        handle.insert(&table_name, key.as_slice(), &[0; 32])?;
        entries.push(key);
    }
    entries.sort();

    let all_entries = handle.range(&table_name, ..)?.collect::<Result<Vec<_>>>()?;
    assert_eq!(all_entries.len(), entries.len());
    for (v0, v1) in all_entries.iter().zip(entries.iter()) {
        assert_eq!(v0.key(), v1);
    }

    let midpoint = entries.get(MIDPOINT).unwrap();
    {
        let upper_entries = handle
            .range(&table_name, midpoint.as_slice()..)?
            .collect::<Result<Vec<_>>>()?;
        assert_eq!(upper_entries.len(), entries[MIDPOINT..].len());
        for (v0, v1) in upper_entries.iter().zip(entries[MIDPOINT..].iter()) {
            assert_eq!(v0.key(), v1);
        }
    }
    {
        let lower_entries = handle
            .range(&table_name, ..midpoint.as_slice())?
            .collect::<Result<Vec<_>>>()?;
        assert_eq!(lower_entries.len(), entries[..MIDPOINT].len());
        for (v0, v1) in lower_entries.iter().zip(entries[..MIDPOINT].iter()) {
            assert_eq!(v0.key(), v1);
        }
    }
    {
        let midpoint = entries.get(50).unwrap();
        let lower_inclusive_entries = handle
            .range(&table_name, ..=midpoint.as_slice())?
            .collect::<Result<Vec<_>>>()?;
        assert_eq!(lower_inclusive_entries.len(), entries[..=MIDPOINT].len());
        for (v0, v1) in lower_inclusive_entries
            .iter()
            .zip(entries[..=MIDPOINT].iter())
        {
            assert_eq!(v0.key(), v1);
        }
    }

    Ok(())
}
