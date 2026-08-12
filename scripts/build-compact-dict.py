# Build the compact dictionary (ecdict-compact-8k.db) used by the in-app cloud download.
# Run from repo root. Requires dictionaries/ecdict.db (full 812MB, gitignored).
# Keeps ALL columns the app queries (exchange/collins/oxford/tag/bnc/...), filters
# frq<=8000, and prints the resulting sha256 + size for src/services/dictDownload.ts.
# Upload the artifact to the ecdict-v1 GitHub Release:
#   gh release upload ecdict-v1 dictionaries/ecdict-compact-8k.db --clobber
import hashlib
import os
import sqlite3

SRC = r'dictionaries/ecdict.db'
OUT = r'dictionaries/ecdict-compact-8k.db'
MAX_FRQ = 8000


def main() -> None:
    src = sqlite3.connect(SRC)
    out = sqlite3.connect(OUT)
    out.executescript(
        '''
        CREATE TABLE stardict (
            id INTEGER PRIMARY KEY,
            word VARCHAR(64) NOT NULL,
            sw VARCHAR(64) NOT NULL,
            phonetic VARCHAR(64),
            definition TEXT,
            translation TEXT,
            pos VARCHAR(16),
            collins INTEGER DEFAULT 0,
            oxford INTEGER DEFAULT 0,
            tag VARCHAR(64),
            bnc INTEGER,
            frq INTEGER,
            exchange TEXT,
            detail TEXT,
            audio TEXT
        );
        CREATE INDEX idx_sw ON stardict(sw);
        CREATE INDEX idx_stardict_frq ON stardict(frq);
        '''
    )
    cols = ('id, word, sw, phonetic, definition, translation, pos, '
            'collins, oxford, tag, bnc, frq, exchange, detail, audio')
    rows = src.execute(
        'SELECT %s FROM stardict WHERE frq IS NOT NULL AND frq <= ? ORDER BY frq' % cols,
        (MAX_FRQ,),
    )
    n = 0
    for row in rows:
        out.execute('INSERT INTO stardict VALUES (' + ','.join(['?'] * 15) + ')', row)
        n += 1
    out.commit()
    out.execute('VACUUM')
    out.commit()
    src.close()

    check = sqlite3.connect(OUT)
    print('rows:', n)
    print('hello:', check.execute(
        "SELECT word, translation, frq FROM stardict WHERE word LIKE 'hello' COLLATE NOCASE"
    ).fetchone())
    print('exchange:', check.execute(
        "SELECT word, exchange FROM stardict WHERE word LIKE 'hello' COLLATE NOCASE"
    ).fetchone())
    print('max_frq:', check.execute('SELECT MAX(frq) FROM stardict').fetchone())
    check.close()

    digest = hashlib.sha256(open(OUT, 'rb').read()).hexdigest()
    print('sha256:', digest)
    print('size_mb:', round(os.path.getsize(OUT) / 1048576, 1))


if __name__ == '__main__':
    main()