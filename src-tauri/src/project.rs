use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use chrono::Utc;
use uuid::Uuid;

/// Translation project data model
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationProject {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source_lang: String,
    pub target_lang: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub status: String, // "active", "completed", "archived"
    pub total_files: i32,
    pub completed_files: i32,
    pub total_segments: i32,
    pub translated_segments: i32,
}

/// Project file entry
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFile {
    pub id: String,
    pub project_id: String,
    pub file_name: String,
    pub file_path: String,
    pub file_type: String, // "txt", "docx", "pdf", "epub", "srt", etc.
    pub file_size: i64,
    pub status: String, // "pending", "translating", "completed", "error"
    pub total_segments: i32,
    pub translated_segments: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Translation segment within a file
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationSegment {
    pub id: String,
    pub file_id: String,
    pub segment_index: i32,
    pub source_text: String,
    pub translated_text: String,
    pub status: String, // "pending", "translated", "reviewed", "approved"
    pub created_at: i64,
    pub updated_at: i64,
}

/// Project store for SQLite operations
pub struct ProjectStore {
    conn: Mutex<Connection>,
}

fn db_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("moontranslator");
    if let Err(e) = std::fs::create_dir_all(&path) {
        tracing::warn!("Failed to create project db directory {:?}: {}", path, e);
    }
    path.push("projects.db");
    path
}

impl ProjectStore {
    pub fn load() -> Self {
        let path = db_path();
        let conn = match Connection::open(&path) {
            Ok(conn) => conn,
            Err(e) => {
                tracing::error!("Failed to open project database: {}", e);
                Connection::open_in_memory().expect("Failed to create in-memory project db")
            }
        };

        // Create tables
        if let Err(e) = conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS projects (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT DEFAULT '',
                source_lang TEXT NOT NULL DEFAULT 'auto',
                target_lang TEXT NOT NULL DEFAULT 'zh',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                status TEXT NOT NULL DEFAULT 'active',
                total_files INTEGER NOT NULL DEFAULT 0,
                completed_files INTEGER NOT NULL DEFAULT 0,
                total_segments INTEGER NOT NULL DEFAULT 0,
                translated_segments INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS project_files (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                file_name TEXT NOT NULL,
                file_path TEXT NOT NULL,
                file_type TEXT NOT NULL,
                file_size INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'pending',
                total_segments INTEGER NOT NULL DEFAULT 0,
                translated_segments INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS translation_segments (
                id TEXT PRIMARY KEY,
                file_id TEXT NOT NULL,
                segment_index INTEGER NOT NULL,
                source_text TEXT NOT NULL,
                translated_text TEXT DEFAULT '',
                status TEXT NOT NULL DEFAULT 'pending',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                FOREIGN KEY (file_id) REFERENCES project_files(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_project_files_project_id ON project_files(project_id);
            CREATE INDEX IF NOT EXISTS idx_translation_segments_file_id ON translation_segments(file_id);
            CREATE INDEX IF NOT EXISTS idx_projects_status ON projects(status);
            CREATE INDEX IF NOT EXISTS idx_projects_updated_at ON projects(updated_at DESC);
        ") {
            tracing::error!("Failed to create project tables: {}", e);
        }

        Self {
            conn: Mutex::new(conn),
        }
    }

    // ==================== Project CRUD ====================

    pub fn create_project(
        &self,
        name: &str,
        description: &str,
        source_lang: &str,
        target_lang: &str,
    ) -> Result<TranslationProject, String> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();

        let project = TranslationProject {
            id: id.clone(),
            name: name.to_string(),
            description: description.to_string(),
            source_lang: source_lang.to_string(),
            target_lang: target_lang.to_string(),
            created_at: now,
            updated_at: now,
            status: "active".to_string(),
            total_files: 0,
            completed_files: 0,
            total_segments: 0,
            translated_segments: 0,
        };

        conn.execute(
            "INSERT INTO projects (id, name, description, source_lang, target_lang, created_at, updated_at, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                project.id,
                project.name,
                project.description,
                project.source_lang,
                project.target_lang,
                project.created_at,
                project.updated_at,
                project.status,
            ],
        )
        .map_err(|e| format!("Failed to create project: {}", e))?;

        Ok(project)
    }

    pub fn get_project(&self, id: &str) -> Result<TranslationProject, String> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());

        conn.query_row(
            "SELECT id, name, description, source_lang, target_lang, created_at, updated_at, status,
                    total_files, completed_files, total_segments, translated_segments
             FROM projects WHERE id = ?1",
            params![id],
            |row| {
                Ok(TranslationProject {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    source_lang: row.get(3)?,
                    target_lang: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                    status: row.get(7)?,
                    total_files: row.get(8)?,
                    completed_files: row.get(9)?,
                    total_segments: row.get(10)?,
                    translated_segments: row.get(11)?,
                })
            },
        )
        .map_err(|e| format!("Project not found: {}", e))
    }

    pub fn get_all_projects(&self) -> Result<Vec<TranslationProject>, String> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn
            .prepare(
                "SELECT id, name, description, source_lang, target_lang, created_at, updated_at, status,
                        total_files, completed_files, total_segments, translated_segments
                 FROM projects ORDER BY updated_at DESC",
            )
            .map_err(|e| format!("Failed to prepare statement: {}", e))?;

        let projects = stmt
            .query_map([], |row| {
                Ok(TranslationProject {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    source_lang: row.get(3)?,
                    target_lang: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                    status: row.get(7)?,
                    total_files: row.get(8)?,
                    completed_files: row.get(9)?,
                    total_segments: row.get(10)?,
                    translated_segments: row.get(11)?,
                })
            })
            .map_err(|e| format!("Failed to query projects: {}", e))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(projects)
    }

    pub fn update_project(
        &self,
        id: &str,
        name: Option<&str>,
        description: Option<&str>,
        source_lang: Option<&str>,
        target_lang: Option<&str>,
        status: Option<&str>,
    ) -> Result<TranslationProject, String> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let now = Utc::now().timestamp();

        // Build dynamic update query
        let mut updates = vec!["updated_at = ?1".to_string()];
        let mut param_index = 2;

        if name.is_some() {
            updates.push(format!("name = ?{}", param_index));
            param_index += 1;
        }
        if description.is_some() {
            updates.push(format!("description = ?{}", param_index));
            param_index += 1;
        }
        if source_lang.is_some() {
            updates.push(format!("source_lang = ?{}", param_index));
            param_index += 1;
        }
        if target_lang.is_some() {
            updates.push(format!("target_lang = ?{}", param_index));
            param_index += 1;
        }
        if status.is_some() {
            updates.push(format!("status = ?{}", param_index));
            param_index += 1;
        }

        let query = format!("UPDATE projects SET {} WHERE id = ?{}", updates.join(", "), param_index);

        // Build params dynamically
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now)];
        if let Some(v) = name {
            param_values.push(Box::new(v.to_string()));
        }
        if let Some(v) = description {
            param_values.push(Box::new(v.to_string()));
        }
        if let Some(v) = source_lang {
            param_values.push(Box::new(v.to_string()));
        }
        if let Some(v) = target_lang {
            param_values.push(Box::new(v.to_string()));
        }
        if let Some(v) = status {
            param_values.push(Box::new(v.to_string()));
        }
        param_values.push(Box::new(id.to_string()));

        let params_ref: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();

        conn.execute(&query, params_ref.as_slice())
            .map_err(|e| format!("Failed to update project: {}", e))?;

        self.get_project(id)
    }

    pub fn delete_project(&self, id: &str) -> Result<(), String> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());

        // Delete project (cascade will handle files and segments)
        conn.execute("DELETE FROM projects WHERE id = ?1", params![id])
            .map_err(|e| format!("Failed to delete project: {}", e))?;

        Ok(())
    }

    // ==================== Project File Operations ====================

    pub fn add_file_to_project(
        &self,
        project_id: &str,
        file_name: &str,
        file_path: &str,
        file_type: &str,
        file_size: i64,
    ) -> Result<ProjectFile, String> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();

        let file = ProjectFile {
            id: id.clone(),
            project_id: project_id.to_string(),
            file_name: file_name.to_string(),
            file_path: file_path.to_string(),
            file_type: file_type.to_string(),
            file_size,
            status: "pending".to_string(),
            total_segments: 0,
            translated_segments: 0,
            created_at: now,
            updated_at: now,
        };

        conn.execute(
            "INSERT INTO project_files (id, project_id, file_name, file_path, file_type, file_size, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                file.id,
                file.project_id,
                file.file_name,
                file.file_path,
                file.file_type,
                file.file_size,
                file.status,
                file.created_at,
                file.updated_at,
            ],
        )
        .map_err(|e| format!("Failed to add file: {}", e))?;

        // Update project file count
        conn.execute(
            "UPDATE projects SET total_files = total_files + 1, updated_at = ?1 WHERE id = ?2",
            params![now, project_id],
        )
        .map_err(|e| format!("Failed to update project file count: {}", e))?;

        Ok(file)
    }

    pub fn get_project_files(&self, project_id: &str) -> Result<Vec<ProjectFile>, String> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn
            .prepare(
                "SELECT id, project_id, file_name, file_path, file_type, file_size, status,
                        total_segments, translated_segments, created_at, updated_at
                 FROM project_files WHERE project_id = ?1 ORDER BY created_at ASC",
            )
            .map_err(|e| format!("Failed to prepare statement: {}", e))?;

        let files = stmt
            .query_map(params![project_id], |row| {
                Ok(ProjectFile {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    file_name: row.get(2)?,
                    file_path: row.get(3)?,
                    file_type: row.get(4)?,
                    file_size: row.get(5)?,
                    status: row.get(6)?,
                    total_segments: row.get(7)?,
                    translated_segments: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            })
            .map_err(|e| format!("Failed to query files: {}", e))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(files)
    }

    pub fn update_file_status(
        &self,
        file_id: &str,
        status: &str,
        total_segments: Option<i32>,
        translated_segments: Option<i32>,
    ) -> Result<ProjectFile, String> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let now = Utc::now().timestamp();

        let mut updates = vec!["updated_at = ?1".to_string()];
        updates.push("status = ?2".to_string());

        if total_segments.is_some() {
            updates.push("total_segments = ?3".to_string());
        }
        if translated_segments.is_some() {
            updates.push("translated_segments = ?4".to_string());
        }

        let query = format!("UPDATE project_files SET {} WHERE id = ?5", updates.join(", "));

        conn.execute(
            &query,
            params![
                now,
                status,
                total_segments.unwrap_or(0),
                translated_segments.unwrap_or(0),
                file_id,
            ],
        )
        .map_err(|e| format!("Failed to update file status: {}", e))?;

        // Get the file to find project_id
        let file = self.get_file(file_id)?;
        self.recalculate_project_progress(&file.project_id)?;

        Ok(file)
    }

    pub fn get_file(&self, file_id: &str) -> Result<ProjectFile, String> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());

        conn.query_row(
            "SELECT id, project_id, file_name, file_path, file_type, file_size, status,
                    total_segments, translated_segments, created_at, updated_at
             FROM project_files WHERE id = ?1",
            params![file_id],
            |row| {
                Ok(ProjectFile {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    file_name: row.get(2)?,
                    file_path: row.get(3)?,
                    file_type: row.get(4)?,
                    file_size: row.get(5)?,
                    status: row.get(6)?,
                    total_segments: row.get(7)?,
                    translated_segments: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            },
        )
        .map_err(|e| format!("File not found: {}", e))
    }

    pub fn delete_file(&self, file_id: &str) -> Result<(), String> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());

        // Get project_id before deleting
        let file = self.get_file(file_id)?;
        let project_id = file.project_id.clone();
        let now = Utc::now().timestamp();

        conn.execute("DELETE FROM project_files WHERE id = ?1", params![file_id])
            .map_err(|e| format!("Failed to delete file: {}", e))?;

        // Update project file count
        conn.execute(
            "UPDATE projects SET total_files = MAX(0, total_files - 1), updated_at = ?1 WHERE id = ?2",
            params![now, project_id],
        )
        .map_err(|e| format!("Failed to update project: {}", e))?;

        self.recalculate_project_progress(&project_id)?;

        Ok(())
    }

    // ==================== Translation Segments ====================

    pub fn add_segments(
        &self,
        file_id: &str,
        segments: Vec<(String, String)>, // (index, source_text)
    ) -> Result<Vec<TranslationSegment>, String> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let now = Utc::now().timestamp();

        let mut result = Vec::new();

        for (index, source_text) in segments {
            let id = Uuid::new_v4().to_string();
            let segment = TranslationSegment {
                id: id.clone(),
                file_id: file_id.to_string(),
                segment_index: index.parse().unwrap_or(0),
                source_text,
                translated_text: String::new(),
                status: "pending".to_string(),
                created_at: now,
                updated_at: now,
            };

            conn.execute(
                "INSERT INTO translation_segments (id, file_id, segment_index, source_text, translated_text, status, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    segment.id,
                    segment.file_id,
                    segment.segment_index,
                    segment.source_text,
                    segment.translated_text,
                    segment.status,
                    segment.created_at,
                    segment.updated_at,
                ],
            )
            .map_err(|e| format!("Failed to add segment: {}", e))?;

            result.push(segment);
        }

        // Update file total_segments
        let total = result.len() as i32;
        conn.execute(
            "UPDATE project_files SET total_segments = total_segments + ?1, updated_at = ?2 WHERE id = ?3",
            params![total, now, file_id],
        )
        .map_err(|e| format!("Failed to update file segments: {}", e))?;

        // Recalculate project progress
        let file = self.get_file(file_id)?;
        self.recalculate_project_progress(&file.project_id)?;

        Ok(result)
    }

    pub fn get_file_segments(&self, file_id: &str) -> Result<Vec<TranslationSegment>, String> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn
            .prepare(
                "SELECT id, file_id, segment_index, source_text, translated_text, status, created_at, updated_at
                 FROM translation_segments WHERE file_id = ?1 ORDER BY segment_index ASC",
            )
            .map_err(|e| format!("Failed to prepare statement: {}", e))?;

        let segments = stmt
            .query_map(params![file_id], |row| {
                Ok(TranslationSegment {
                    id: row.get(0)?,
                    file_id: row.get(1)?,
                    segment_index: row.get(2)?,
                    source_text: row.get(3)?,
                    translated_text: row.get(4)?,
                    status: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            })
            .map_err(|e| format!("Failed to query segments: {}", e))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(segments)
    }

    pub fn update_segment(
        &self,
        segment_id: &str,
        translated_text: &str,
        status: &str,
    ) -> Result<TranslationSegment, String> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let now = Utc::now().timestamp();

        conn.execute(
            "UPDATE translation_segments SET translated_text = ?1, status = ?2, updated_at = ?3 WHERE id = ?4",
            params![translated_text, status, now, segment_id],
        )
        .map_err(|e| format!("Failed to update segment: {}", e))?;

        // Get segment to find file_id
        let segment = conn
            .query_row(
                "SELECT id, file_id, segment_index, source_text, translated_text, status, created_at, updated_at
                 FROM translation_segments WHERE id = ?1",
                params![segment_id],
                |row| {
                    Ok(TranslationSegment {
                        id: row.get(0)?,
                        file_id: row.get(1)?,
                        segment_index: row.get(2)?,
                        source_text: row.get(3)?,
                        translated_text: row.get(4)?,
                        status: row.get(5)?,
                        created_at: row.get(6)?,
                        updated_at: row.get(7)?,
                    })
                },
            )
            .map_err(|e| format!("Segment not found: {}", e))?;

        // Update file translated_segments count
        let translated_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM translation_segments WHERE file_id = ?1 AND status != 'pending'",
                params![segment.file_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        conn.execute(
            "UPDATE project_files SET translated_segments = ?1, updated_at = ?2 WHERE id = ?3",
            params![translated_count, now, segment.file_id],
        )
        .map_err(|e| format!("Failed to update file progress: {}", e))?;

        // Update file status if all segments translated
        let total_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM translation_segments WHERE file_id = ?1",
                params![segment.file_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let new_status = if translated_count >= total_count && total_count > 0 {
            "completed"
        } else if translated_count > 0 {
            "translating"
        } else {
            "pending"
        };

        conn.execute(
            "UPDATE project_files SET status = ?1 WHERE id = ?2",
            params![new_status, segment.file_id],
        )
        .map_err(|e| format!("Failed to update file status: {}", e))?;

        // Recalculate project progress
        let file = self.get_file(&segment.file_id)?;
        self.recalculate_project_progress(&file.project_id)?;

        Ok(segment)
    }

    // ==================== Progress Calculation ====================

    fn recalculate_project_progress(&self, project_id: &str) -> Result<(), String> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let now = Utc::now().timestamp();

        // Calculate totals from files
        let (total_files, completed_files, total_segments, translated_segments): (i32, i32, i32, i32) = conn
            .query_row(
                "SELECT
                    COUNT(*),
                    SUM(CASE WHEN status = 'completed' THEN 1 ELSE 0 END),
                    COALESCE(SUM(total_segments), 0),
                    COALESCE(SUM(translated_segments), 0)
                 FROM project_files WHERE project_id = ?1",
                params![project_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap_or((0, 0, 0, 0));

        conn.execute(
            "UPDATE projects SET
                total_files = ?1,
                completed_files = ?2,
                total_segments = ?3,
                translated_segments = ?4,
                updated_at = ?5
             WHERE id = ?6",
            params![
                total_files,
                completed_files,
                total_segments,
                translated_segments,
                now,
                project_id,
            ],
        )
        .map_err(|e| format!("Failed to update project progress: {}", e))?;

        Ok(())
    }

    // ==================== Export ====================

    pub fn get_project_export_data(&self, project_id: &str) -> Result<ProjectExportData, String> {
        let project = self.get_project(project_id)?;
        let files = self.get_project_files(project_id)?;

        let mut file_exports = Vec::new();
        for file in &files {
            let segments = self.get_file_segments(&file.id)?;
            file_exports.push(FileExportData {
                file_name: file.file_name.clone(),
                file_type: file.file_type.clone(),
                segments,
            });
        }

        Ok(ProjectExportData {
            project,
            files: file_exports,
            exported_at: Utc::now().timestamp(),
        })
    }
}

/// Export data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectExportData {
    pub project: TranslationProject,
    pub files: Vec<FileExportData>,
    pub exported_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileExportData {
    pub file_name: String,
    pub file_type: String,
    pub segments: Vec<TranslationSegment>,
}
