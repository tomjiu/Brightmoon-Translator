// 词典加载服务 - 从 GitHub 加载 ECDICT 分片

export interface DictEntry {
  word: string;
  phonetic?: string;
  definition?: string;
  translation?: string;
  pos?: string;
  frq?: number;
}

export interface Manifest {
  version: string;
  createdAt: string;
  totalWords: number;
  shards: Array<{
    letter: string;
    wordCount: number;
    fileName: string;
  }>;
}

export class DictLoader {
  private repo: string;
  private token: string;
  private cache: KVNamespace;

  constructor(repo: string, token: string, cache: KVNamespace) {
    this.repo = repo;
    this.token = token;
    this.cache = cache;
  }

  // 获取清单文件
  async getManifest(): Promise<Manifest | null> {
    const cacheKey = 'dict:manifest';
    const cached = await this.cache.get(cacheKey, 'json');
    if (cached) {
      return cached as Manifest;
    }

    const url = `https://raw.githubusercontent.com/${this.repo}/main/ecdict/manifest.json`;
    const response = await fetch(url, {
      headers: this.token ? { Authorization: `token ${this.token}` } : {},
    });

    if (!response.ok) {
      return null;
    }

    const manifest = await response.json() as Manifest;
    await this.cache.put(cacheKey, JSON.stringify(manifest), {
      expirationTtl: 3600,
    });

    return manifest;
  }

  // 加载指定字母的分片
  async loadShard(letter: string): Promise<DictEntry[]> {
    const cacheKey = `dict:shard:${letter}`;
    const cached = await this.cache.get(cacheKey, 'json');
    if (cached) {
      return cached as DictEntry[];
    }

    const url = `https://raw.githubusercontent.com/${this.repo}/main/ecdict/ecdict_${letter}.json.gz`;
    const response = await fetch(url, {
      headers: this.token ? { Authorization: `token ${this.token}` } : {},
    });

    if (!response.ok) {
      return [];
    }

    // 解压 gz（Workers 支持 DecompressionStream）
    const ds = new DecompressionStream('gzip');
    const decompressed = response.body!.pipeThrough(ds);
    const text = await new Response(decompressed).text();
    const entries = JSON.parse(text) as DictEntry[];

    // 缓存 7 天
    await this.cache.put(cacheKey, JSON.stringify(entries), {
      expirationTtl: 7 * 24 * 3600,
    });

    return entries;
  }

  // 查询单词
  async lookup(word: string): Promise<DictEntry | null> {
    const letter = word[0].toLowerCase();
    const entries = await this.loadShard(letter);

    return entries.find((e) => e.word === word.toLowerCase()) || null;
  }

  // 搜索建议
  async suggest(prefix: string, limit: number = 10): Promise<DictEntry[]> {
    const letter = prefix[0].toLowerCase();
    const entries = await this.loadShard(letter);

    return entries
      .filter((e) => e.word.startsWith(prefix.toLowerCase()))
      .slice(0, limit);
  }
}
