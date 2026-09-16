//! カード台帳 (`ic_id` テーブル) の読み出しのうち、DB に触らない部分。
//!
//! 生存判定の条件は **このモジュールだけ** に置く ([`ALIVE_SQL`] / [`is_alive`])。
//! SQL 側とRust 側で条件がずれないよう、増やす場合もここを直す。
//!
//! 注意: `src/services/ic_log.rs` の `ic_id` 読み出しは `deleted = 0` のままで、
//! ここの条件とは意図的に異なる (あちらを揃えると既存の打刻解決の挙動が変わるため)。

use std::collections::BTreeMap;

use chrono::NaiveDateTime;

/// 台帳の行を「生存している」とみなす SQL 述語。
///
/// 台帳に行を入れる別クライアントは 3 列しか INSERT しないため、`deleted` が
/// NULL のままの行が在り得る。`deleted = 0` だけで絞るとそれらを取りこぼす。
///
/// 単一テーブルのスコープで使う前提なので列名は修飾しない。
pub const ALIVE_SQL: &str = "(deleted = 0 OR deleted IS NULL)";

/// chunk_size の既定値。
pub const DEFAULT_CHUNK_SIZE: usize = 500;

/// chunk_size の上限。
pub const MAX_CHUNK_SIZE: usize = 5000;

/// `ic_id` テーブルの 1 行 (畳み込み前)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerRow {
    pub ic_id: String,
    pub emp_id: Option<i32>,
    pub date: Option<NaiveDateTime>,
    pub deleted: Option<i8>,
}

/// 1 枚のカードにつき 1 件に畳み込んだ結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerEntry {
    pub ic_id: String,
    pub emp_id: Option<i32>,
    pub date: Option<NaiveDateTime>,
}

/// [`ALIVE_SQL`] の Rust 版。
pub fn is_alive(deleted: Option<i8>) -> bool {
    matches!(deleted, None | Some(0))
}

/// リクエストの chunk_size を既定値・上限に丸める。
pub fn normalize_chunk_size(requested: Option<i32>) -> usize {
    match requested {
        Some(n) if n > 0 => (n as usize).min(MAX_CHUNK_SIZE),
        _ => DEFAULT_CHUNK_SIZE,
    }
}

/// 生存判定・空 `ic_id` の除外・同一 `ic_id` の MAX(date) 畳み込みを行い、
/// `ic_id` の昇順で返す。
///
/// `ic_id` の値は DB のまま返す (小文字化も区切り除去もしない)。正規化は
/// 受け取り側の責務で、そこに 1 か所だけ置く。
pub fn fold_rows(rows: Vec<LedgerRow>) -> Vec<LedgerEntry> {
    let mut latest: BTreeMap<String, LedgerRow> = BTreeMap::new();

    for row in rows {
        if row.ic_id.is_empty() || !is_alive(row.deleted) {
            continue;
        }
        // date が NULL の行は `None < Some(_)` により、日時のある行に負ける
        // (SQL の MAX() が NULL を無視するのと同じ)。
        match latest.get(&row.ic_id) {
            Some(kept) if kept.date >= row.date => {}
            _ => {
                latest.insert(row.ic_id.clone(), row);
            }
        }
    }

    latest
        .into_values()
        .map(|row| LedgerEntry {
            ic_id: row.ic_id,
            emp_id: row.emp_id,
            date: row.date,
        })
        .collect()
}

/// 畳み込み済みの列を 1 chunk に切る。
///
/// 呼び出し側は `chunk_size + 1` 件ぶん取ってくる前提で、溢れた分があれば
/// 「続きがある」とみなして次の cursor (返した最後の `ic_id`) を添える。
pub fn take_chunk(
    mut entries: Vec<LedgerEntry>,
    chunk_size: usize,
) -> (Vec<LedgerEntry>, Option<String>) {
    if entries.len() <= chunk_size {
        return (entries, None);
    }

    entries.truncate(chunk_size);
    let next_cursor = entries.last().map(|e| e.ic_id.clone());
    (entries, next_cursor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn dt(day: u32, hour: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2024, 1, day)
            .unwrap()
            .and_hms_opt(hour, 0, 0)
            .unwrap()
    }

    fn row(
        ic_id: &str,
        emp_id: i32,
        date: Option<NaiveDateTime>,
        deleted: Option<i8>,
    ) -> LedgerRow {
        LedgerRow {
            ic_id: ic_id.to_string(),
            emp_id: Some(emp_id),
            date,
            deleted,
        }
    }

    #[test]
    fn drops_deleted_rows() {
        let folded = fold_rows(vec![
            row("CARD0001", 1, Some(dt(1, 9)), Some(1)),
            row("CARD0002", 2, Some(dt(1, 9)), Some(0)),
        ]);

        assert_eq!(
            folded.iter().map(|e| e.ic_id.as_str()).collect::<Vec<_>>(),
            vec!["CARD0002"]
        );
    }

    #[test]
    fn keeps_rows_with_null_deleted() {
        // 台帳に 3 列しか INSERT しないクライアントが作る行。
        let folded = fold_rows(vec![row("CARD0001", 1, Some(dt(1, 9)), None)]);

        assert_eq!(folded.len(), 1);
        assert_eq!(folded[0].ic_id, "CARD0001");
        assert_eq!(folded[0].emp_id, Some(1));
    }

    #[test]
    fn is_alive_matches_the_sql_predicate() {
        assert!(is_alive(None));
        assert!(is_alive(Some(0)));
        assert!(!is_alive(Some(1)));
        assert!(ALIVE_SQL.contains("deleted = 0"));
        assert!(ALIVE_SQL.contains("deleted IS NULL"));
    }

    #[test]
    fn keeps_only_the_row_with_max_date() {
        let folded = fold_rows(vec![
            row("CARD0001", 10, Some(dt(1, 9)), Some(0)),
            row("CARD0001", 30, Some(dt(3, 9)), None),
            row("CARD0001", 20, Some(dt(2, 9)), Some(0)),
        ]);

        assert_eq!(folded.len(), 1);
        assert_eq!(folded[0].emp_id, Some(30));
        assert_eq!(folded[0].date, Some(dt(3, 9)));
    }

    #[test]
    fn a_null_date_loses_to_a_dated_row() {
        let folded = fold_rows(vec![
            row("CARD0001", 10, None, None),
            row("CARD0001", 20, Some(dt(1, 9)), None),
        ]);

        assert_eq!(folded.len(), 1);
        assert_eq!(folded[0].emp_id, Some(20));
    }

    #[test]
    fn drops_rows_with_an_empty_ic_id() {
        let folded = fold_rows(vec![
            row("", 1, Some(dt(1, 9)), Some(0)),
            row("", 2, Some(dt(2, 9)), None),
            row("CARD0001", 3, Some(dt(1, 9)), None),
        ]);

        assert_eq!(folded.len(), 1);
        assert_eq!(folded[0].ic_id, "CARD0001");
    }

    #[test]
    fn preserves_the_ic_id_verbatim() {
        // 正規化 (小文字化・区切り除去) は受け取り側の責務。
        let folded = fold_rows(vec![row("AB:CD-01ef", 1, Some(dt(1, 9)), None)]);

        assert_eq!(folded[0].ic_id, "AB:CD-01ef");
    }

    #[test]
    fn folds_in_ic_id_order() {
        let folded = fold_rows(vec![
            row("CARD0003", 3, Some(dt(1, 9)), None),
            row("CARD0001", 1, Some(dt(1, 9)), None),
            row("CARD0002", 2, Some(dt(1, 9)), None),
        ]);

        assert_eq!(
            folded.iter().map(|e| e.ic_id.as_str()).collect::<Vec<_>>(),
            vec!["CARD0001", "CARD0002", "CARD0003"]
        );
    }

    fn entries(n: usize) -> Vec<LedgerEntry> {
        (0..n)
            .map(|i| LedgerEntry {
                ic_id: format!("CARD{:04}", i),
                emp_id: Some(i as i32),
                date: Some(dt(1, 9)),
            })
            .collect()
    }

    #[test]
    fn splits_at_the_chunk_boundary() {
        // chunk_size + 1 件取れた = 続きがある。
        let (chunk, next) = take_chunk(entries(501), DEFAULT_CHUNK_SIZE);

        assert_eq!(chunk.len(), 500);
        assert_eq!(chunk.last().unwrap().ic_id, "CARD0499");
        assert_eq!(next, Some("CARD0499".to_string()));
    }

    #[test]
    fn does_not_split_when_the_chunk_is_exactly_full() {
        let (chunk, next) = take_chunk(entries(500), DEFAULT_CHUNK_SIZE);

        assert_eq!(chunk.len(), 500);
        assert_eq!(next, None);
    }

    #[test]
    fn does_not_split_the_last_chunk() {
        let (chunk, next) = take_chunk(entries(3), DEFAULT_CHUNK_SIZE);

        assert_eq!(chunk.len(), 3);
        assert_eq!(next, None);
    }

    #[test]
    fn normalizes_the_chunk_size() {
        assert_eq!(normalize_chunk_size(None), DEFAULT_CHUNK_SIZE);
        assert_eq!(normalize_chunk_size(Some(0)), DEFAULT_CHUNK_SIZE);
        assert_eq!(normalize_chunk_size(Some(-1)), DEFAULT_CHUNK_SIZE);
        assert_eq!(normalize_chunk_size(Some(10)), 10);
        assert_eq!(normalize_chunk_size(Some(i32::MAX)), MAX_CHUNK_SIZE);
    }
}
