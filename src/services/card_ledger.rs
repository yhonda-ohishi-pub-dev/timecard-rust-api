// gRPC CardLedgerService implementation
// カード台帳 (ic_id テーブル) を ic_id 昇順の keyset ページングで列挙する。
//
// 生存判定・畳み込み・chunk 分割の規則は crate::card_ledger に置いてある。

use crate::card_ledger::{self, LedgerRow};
use crate::db::Database;
use crate::proto::timecard::{
    card_ledger_service_server::CardLedgerService, CardLedgerChunk, CardLedgerEntry,
    ListCardsRequest,
};
use sqlx::mysql::MySqlRow;
use sqlx::Row;
use tonic::{Request, Response, Status};
use tracing::info;

pub struct CardLedgerServiceImpl {
    db: Database,
}

impl CardLedgerServiceImpl {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// cursor より後ろの ic_id を `limit` 枚ぶん選び、その全行を返す SQL。
    ///
    /// LIMIT は「行」ではなく「カードの枚数」にかかるので、同じ ic_id の行が
    /// chunk の境界で分断されない。
    fn page_sql() -> String {
        format!(
            "SELECT ic_id, emp_id, CAST(date AS DATETIME) AS date, deleted
             FROM ic_id
             WHERE {alive}
               AND ic_id IN (
                   SELECT ic_id FROM (
                       SELECT DISTINCT ic_id
                       FROM ic_id
                       WHERE {alive} AND ic_id <> '' AND ic_id > ?
                       ORDER BY ic_id
                       LIMIT ?
                   ) AS page
               )
             ORDER BY ic_id",
            alive = card_ledger::ALIVE_SQL
        )
    }

    /// emp_id の列型が INT でも VARCHAR でも拾えるようにする。
    /// (この DB には iid のように VARCHAR で持たれている ID 列の前例がある)
    fn emp_id_of(row: &MySqlRow) -> Option<i32> {
        if let Ok(v) = row.try_get::<i32, _>("emp_id") {
            return Some(v);
        }
        row.try_get::<String, _>("emp_id")
            .ok()
            .and_then(|v| v.trim().parse().ok())
    }
}

#[tonic::async_trait]
impl CardLedgerService for CardLedgerServiceImpl {
    async fn list_cards(
        &self,
        request: Request<ListCardsRequest>,
    ) -> Result<Response<CardLedgerChunk>, Status> {
        let req = request.into_inner();
        let chunk_size = card_ledger::normalize_chunk_size(req.chunk_size);
        let cursor = req.cursor.unwrap_or_default();

        // 続きがあるかを判定するため 1 枚多く取る。
        let page_size = chunk_size as i64 + 1;

        let rows = sqlx::query(&Self::page_sql())
            .bind(&cursor)
            .bind(page_size)
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let rows: Vec<LedgerRow> = rows
            .iter()
            .map(|row| LedgerRow {
                ic_id: row.try_get("ic_id").unwrap_or_default(),
                emp_id: Self::emp_id_of(row),
                date: row.try_get("date").ok(),
                deleted: row.try_get("deleted").ok(),
            })
            .collect();

        let (folded, next_cursor) =
            card_ledger::take_chunk(card_ledger::fold_rows(rows), chunk_size);

        // ic_id の値はログに出さない (カードの識別子そのもののため)。
        info!(
            "CardLedger.ListCards: {} entries, has_more={}",
            folded.len(),
            next_cursor.is_some()
        );

        let entries: Vec<CardLedgerEntry> = folded
            .into_iter()
            .map(|e| CardLedgerEntry {
                ic_id: e.ic_id,
                emp_id: e.emp_id,
                date: e.date.map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string()),
            })
            .collect();

        Ok(Response::new(CardLedgerChunk {
            entries,
            next_cursor,
            chunk_size: chunk_size as i32,
        }))
    }
}
