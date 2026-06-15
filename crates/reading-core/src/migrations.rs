//! 最简 SQLite schema 迁移框架（DECISIONS.md 2026-06-13 / 桥接协议文档 v0.5 前置）。
//!
//! 每个数据库文件用 `PRAGMA user_version` 记录「已应用到的版本号」；`run` 按版本
//! 升序执行所有「版本号 > 当前」的迁移，每条迁移在独立事务里跑完并原子地推进
//! user_version——任一条失败整体回滚，不会留下半截 schema。
//!
//! 基线迁移（version 1）刻意用 `CREATE TABLE IF NOT EXISTS`：迁移框架上线前就已存在
//! 的旧库（books 已建、user_version 可能为 0 或 1）也能被幂等补盖并正确盖戳，不报错。
//!
//! v0.5 把 books 单表迁到 series/volume/edition/asset 时，只需向对应数据库的迁移数组
//! 追加 version 2 的脚本——标注与阅读进度以内容哈希为键，不在此迁移范围内、保持不动。

use rusqlite::Connection;

/// 一条有序迁移：把数据库从 `version - 1` 推进到 `version`。
pub struct Migration {
    pub version: u32,
    pub sql: &'static str,
}

/// 当前已应用的 schema 版本（即 `PRAGMA user_version`，新库为 0）。
pub fn current_version(conn: &Connection) -> rusqlite::Result<u32> {
    conn.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
        .map(|v| v as u32)
}

/// 按版本升序应用所有尚未应用的迁移。`migrations` 必须按 `version` 升序排列。
pub fn run(conn: &Connection, migrations: &[Migration]) -> rusqlite::Result<()> {
    let mut current = current_version(conn)?;
    let mut prev = 0u32; // 校验数组自身按 version 严格升序，与 DB 当前版本无关
    for m in migrations {
        debug_assert!(m.version > prev, "迁移数组必须按 version 严格升序、不重复");
        prev = m.version;
        if m.version <= current {
            continue;
        }
        conn.execute_batch("BEGIN")?;
        // user_version 不支持绑定参数；version 来自代码内常量，无注入风险。
        let applied = conn
            .execute_batch(m.sql)
            .and_then(|()| conn.execute_batch(&format!("PRAGMA user_version = {}", m.version)));
        match applied {
            Ok(()) => {
                conn.execute_batch("COMMIT")?;
                current = m.version;
            }
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                return Err(e);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem() -> Connection {
        Connection::open_in_memory().unwrap()
    }

    fn table_exists(conn: &Connection, name: &str) -> bool {
        conn.query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1",
            [name],
            |_| Ok(()),
        )
        .is_ok()
    }

    #[test]
    fn fresh_db_applies_all_and_stamps_version() {
        let conn = mem();
        let migs = [
            Migration { version: 1, sql: "CREATE TABLE a(x);" },
            Migration { version: 2, sql: "CREATE TABLE b(y);" },
        ];
        run(&conn, &migs).unwrap();
        assert_eq!(current_version(&conn).unwrap(), 2);
        assert!(table_exists(&conn, "a"));
        assert!(table_exists(&conn, "b"));
    }

    #[test]
    fn rerun_is_idempotent() {
        let conn = mem();
        let migs = [Migration { version: 1, sql: "CREATE TABLE a(x);" }];
        run(&conn, &migs).unwrap();
        // 第二次跑：v1 已应用，被跳过；不会因 CREATE 重复而报错。
        run(&conn, &migs).unwrap();
        assert_eq!(current_version(&conn).unwrap(), 1);
    }

    #[test]
    fn incremental_upgrade_runs_only_new_steps() {
        let conn = mem();
        run(&conn, &[Migration { version: 1, sql: "CREATE TABLE a(x);" }]).unwrap();
        assert_eq!(current_version(&conn).unwrap(), 1);

        // 追加 v2：只跑 v2，v1 不重跑。
        run(
            &conn,
            &[
                Migration { version: 1, sql: "CREATE TABLE a(x);" },
                Migration { version: 2, sql: "ALTER TABLE a ADD COLUMN y;" },
            ],
        )
        .unwrap();
        assert_eq!(current_version(&conn).unwrap(), 2);
        // y 列存在 → v2 确实执行
        conn.execute("INSERT INTO a(x, y) VALUES (1, 2)", []).unwrap();
    }

    #[test]
    fn preexisting_tables_at_version_zero_get_stamped() {
        // 模拟迁移框架上线前的旧库：表已建、user_version 仍是 0。
        let conn = mem();
        conn.execute_batch("CREATE TABLE a(x);").unwrap();
        assert_eq!(current_version(&conn).unwrap(), 0);

        // 基线迁移用 IF NOT EXISTS → 幂等补盖并盖戳到 1，不报错。
        run(
            &conn,
            &[Migration { version: 1, sql: "CREATE TABLE IF NOT EXISTS a(x);" }],
        )
        .unwrap();
        assert_eq!(current_version(&conn).unwrap(), 1);
    }

    #[test]
    fn failed_migration_rolls_back_and_keeps_version() {
        let conn = mem();
        run(&conn, &[Migration { version: 1, sql: "CREATE TABLE a(x);" }]).unwrap();

        // v2 先建表再撞上非法语句 → 整条回滚：版本停在 1，b 表不应残留。
        let err = run(
            &conn,
            &[
                Migration { version: 1, sql: "CREATE TABLE a(x);" },
                Migration { version: 2, sql: "CREATE TABLE b(y); THIS IS NOT SQL;" },
            ],
        );
        assert!(err.is_err());
        assert_eq!(current_version(&conn).unwrap(), 1);
        assert!(!table_exists(&conn, "b"), "失败迁移建的表应被回滚");
    }
}
