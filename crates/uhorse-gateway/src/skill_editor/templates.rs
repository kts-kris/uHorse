//! Skill editor HTML templates

/// Main skill editor HTML
pub fn skill_editor_html() -> String {
    skill_editor_html_with_skill("")
}

/// Skill editor HTML with specific skill loaded
pub fn skill_editor_html_with_skill(skill_name: &str) -> String {
    format!(
        r##"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Skill Editor - uHorse</title>
    <link rel="stylesheet" href="/static/css/skill-editor.css">
    <link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/codemirror/5.65.15/codemirror.min.css">
    <link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/codemirror/5.65.15/theme/dracula.min.css">
</head>
<body>
    <div class="app">
        <!-- Sidebar -->
        <aside class="sidebar">
            <div class="sidebar-header">
                <h2>📚 Skills</h2>
                <button id="new-skill-btn" class="btn btn-primary">+ New</button>
            </div>
            <div class="skill-list" id="skill-list">
                <div class="loading">Loading...</div>
            </div>
        </aside>

        <!-- Main Editor -->
        <main class="editor-main">
            <div class="editor-header">
                <div class="skill-name">
                    <input type="text" id="skill-name" placeholder="Skill name" value="{skill_name}">
                </div>
                <div class="editor-actions">
                    <button id="validate-btn" class="btn">✓ Validate</button>
                    <button id="save-btn" class="btn btn-primary">💾 Save</button>
                    <button id="delete-btn" class="btn btn-danger">🗑️ Delete</button>
                </div>
            </div>

            <div class="editor-container">
                <!-- Validation Panel -->
                <div class="validation-panel" id="validation-panel">
                    <h3>Validation</h3>
                    <ul id="validation-results"></ul>
                </div>

                <!-- Editor -->
                <div class="editor-wrapper">
                    <textarea id="skill-editor"></textarea>
                </div>

                <!-- Preview -->
                <div class="preview-panel">
                    <h3>Preview</h3>
                    <div id="preview-content"></div>
                </div>
            </div>
        </main>
    </div>

    <!-- New Skill Modal -->
    <div class="modal" id="new-skill-modal">
        <div class="modal-content">
            <h3>Create New Skill</h3>
            <div class="form-group">
                <label for="new-skill-name">Skill Name</label>
                <input type="text" id="new-skill-name" placeholder="my-skill">
            </div>
            <div class="form-group">
                <label for="new-skill-template">Template</label>
                <select id="new-skill-template">
                    <option value="">Empty</option>
                    <option value="basic">Basic</option>
                    <option value="api">API Call</option>
                    <option value="calculator">Calculator</option>
                    <option value="search">Web Search</option>
                </select>
            </div>
            <div class="modal-actions">
                <button class="btn" id="modal-cancel">Cancel</button>
                <button class="btn btn-primary" id="modal-create">Create</button>
            </div>
        </div>
    </div>

    <script src="https://cdnjs.cloudflare.com/ajax/libs/codemirror/5.65.15/codemirror.min.js"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/codemirror/5.65.15/mode/markdown/markdown.min.js"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/codemirror/5.65.15/mode/javascript/javascript.min.js"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/marked/9.1.6/marked.min.js"></script>
    <script src="/static/js/skill-editor.js"></script>
    <script>
        // Initialize with skill name if provided
        if ('{skill_name}') {{
            window.initialSkill = '{skill_name}';
        }}
    </script>
</body>
</html>"##,
        skill_name = skill_name
    )
}

/// Skill editor CSS
pub fn skill_editor_css() -> String {
    include_str!("static/css/skill-editor.css").to_string()
}

/// Skill editor JavaScript
pub fn skill_editor_js() -> String {
    include_str!("static/js/skill-editor.js").to_string()
}
