"""SQLite 是唯一配置源；JSON 文件只用于首次导入和显式导出。"""
from __future__ import annotations
import json
import sqlite3
from .config import default_config, validate_config


class Conflict(Exception):
    pass


class Store:
    def __init__(self, path, initial=None):
        self.db = sqlite3.connect(path, check_same_thread=False)
        try:
            self._initialize(initial)
        except Exception:
            self.db.close()
            raise

    def _initialize(self, initial):
        self.db.execute("PRAGMA journal_mode=WAL")
        self.db.execute("PRAGMA synchronous=FULL")
        self.db.executescript("""
            CREATE TABLE IF NOT EXISTS config (
                id INTEGER PRIMARY KEY CHECK(id=1), revision INTEGER NOT NULL, body TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS actions (
                name TEXT PRIMARY KEY, revision INTEGER NOT NULL, body TEXT);
        """)
        if not self.db.execute("SELECT 1 FROM config").fetchone():
            c = default_config() if initial is None else initial
            validate_config(c)
            with self.db:
                self.db.execute("INSERT INTO config VALUES(1,1,?)", (self.encode(c),))
        elif initial is not None:
            raise ValueError("数据库已存在；修改配置请使用 UpdateConfig")
        validate_config(self.config()[1])

    @staticmethod
    def encode(value):
        return json.dumps(value, ensure_ascii=False, allow_nan=False, separators=(",", ":"))

    def config(self):
        rev, body = self.db.execute("SELECT revision,body FROM config WHERE id=1").fetchone()
        return rev, json.loads(body)

    def update_config(self, expected, value):
        validate_config(value)
        with self.db:
            cursor = self.db.execute("UPDATE config SET revision=revision+1,body=? "
                                     "WHERE id=1 AND revision=?", (self.encode(value), expected))
            if cursor.rowcount != 1:
                raise Conflict("配置已被修改，请重新读取 revision")
        return self.config()

    def actions(self):
        return [(rev, json.loads(body)) for rev, body in self.db.execute(
            "SELECT revision,body FROM actions WHERE body IS NOT NULL ORDER BY name")]

    def action_versions(self):
        return [(name, rev, json.loads(body) if body is not None else None)
                for name, rev, body in self.db.execute(
                    "SELECT name,revision,body FROM actions ORDER BY name")]

    def action(self, name):
        row = self.db.execute("SELECT revision,body FROM actions WHERE name=?", (name,)).fetchone()
        if row is None or row[1] is None:
            raise KeyError(name)
        return row[0], json.loads(row[1])

    def put_action(self, name, expected, value):
        with self.db:
            row = self.db.execute("SELECT revision FROM actions WHERE name=?", (name,)).fetchone()
            revision = row[0] if row else 0
            if revision != expected:
                raise Conflict("动作版本不一致")
            self.db.execute("INSERT INTO actions VALUES(?,?,?) ON CONFLICT(name) DO UPDATE "
                            "SET revision=excluded.revision,body=excluded.body",
                            (name, revision + 1, self.encode(value)))
        return revision + 1

    def delete_action(self, name, expected):
        # 保留删除后的版本，旧客户端不能用过期 revision 重新覆盖同名动作。
        with self.db:
            cursor = self.db.execute("UPDATE actions SET revision=revision+1,body=NULL "
                                     "WHERE name=? AND revision=? AND body IS NOT NULL",
                                     (name, expected))
            if cursor.rowcount != 1:
                raise Conflict("动作不存在或版本不一致")
        return expected + 1

    def close(self):
        self.db.close()
