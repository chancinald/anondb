use futures::TryStreamExt;

use super::*;

#[domacro(all_kv_impls)]
async fn range_equal_len<T: KV>(handle: &T) -> Result<()> {
    const LEN: usize = 100;
    const MIDPOINT: usize = LEN / 2;

    let table_name = rand_utf8(10);
    let mut entries = Vec::<[u8; 32]>::default();
    for _ in 0..LEN {
        let key = rand::random();
        entries.push(key);
        handle.insert(&table_name, key.as_slice(), &[0; 32]).await?;
    }
    entries.sort();
    let read_tx = handle.read_tx().await?;

    let query = read_tx.range(&table_name, ..).await?;
    let stream = std::pin::pin!(query);
    let all_entries = stream.try_collect::<Vec<_>>().await?;
    assert_eq!(all_entries.len(), entries.len());
    for (v0, v1) in all_entries.iter().zip(entries.iter()) {
        assert_eq!(v0.key(), v1);
    }

    let midpoint = entries.get(MIDPOINT).unwrap();
    {
        let query = read_tx.range(&table_name, midpoint.to_vec()..).await?;
        let stream = std::pin::pin!(query);
        let upper_entries = stream.try_collect::<Vec<_>>().await?;
        assert_eq!(upper_entries.len(), entries[MIDPOINT..].len());
        for (v0, v1) in upper_entries.iter().zip(entries[MIDPOINT..].iter()) {
            assert_eq!(v0.key(), v1);
        }
    }
    {
        let query = read_tx.range(&table_name, ..midpoint.to_vec()).await?;
        let stream = std::pin::pin!(query);
        let lower_entries = stream.try_collect::<Vec<_>>().await?;
        assert_eq!(lower_entries.len(), entries[..MIDPOINT].len());
        for (v0, v1) in lower_entries.iter().zip(entries[..MIDPOINT].iter()) {
            assert_eq!(v0.key(), v1);
        }
    }
    {
        let midpoint = entries.get(50).unwrap();
        let midpoint = midpoint.to_vec();
        let query = read_tx.range(&table_name, ..=midpoint).await?;
        let stream = std::pin::pin!(query);
        let lower_inclusive_entries = stream.try_collect::<Vec<_>>().await?;
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
