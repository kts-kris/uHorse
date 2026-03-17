/**
 * Skill Editor JavaScript
 */

class SkillEditor {
    constructor() {
        this.currentSkill = null;
        this.editor = null;
        this.skills = [];
        this.templates = [];

        this.init();
    }

    async init() {
        this.initEditor();
        this.bindEvents();
        await this.loadSkills();
        await this.loadTemplates();

        // Load initial skill if specified
        if (window.initialSkill) {
            await this.loadSkill(window.initialSkill);
        }
    }

    initEditor() {
        const textarea = document.getElementById('skill-editor');
        this.editor = CodeMirror.fromTextArea(textarea, {
            mode: 'markdown',
            theme: 'dracula',
            lineNumbers: true,
            lineWrapping: true,
            indentUnit: 4,
            tabSize: 4,
            autofocus: true,
        });

        // Auto-preview on change
        let debounceTimer;
        this.editor.on('change', () => {
            clearTimeout(debounceTimer);
            debounceTimer = setTimeout(() => {
                this.updatePreview();
            }, 500);
        });
    }

    bindEvents() {
        // New skill button
        document.getElementById('new-skill-btn').addEventListener('click', () => {
            this.showNewSkillModal();
        });

        // Modal actions
        document.getElementById('modal-cancel').addEventListener('click', () => {
            this.hideNewSkillModal();
        });

        document.getElementById('modal-create').addEventListener('click', () => {
            this.createNewSkill();
        });

        // Editor actions
        document.getElementById('validate-btn').addEventListener('click', () => {
            this.validateSkill();
        });

        document.getElementById('save-btn').addEventListener('click', () => {
            this.saveSkill();
        });

        document.getElementById('delete-btn').addEventListener('click', () => {
            this.deleteSkill();
        });

        // Keyboard shortcuts
        document.addEventListener('keydown', (e) => {
            if ((e.ctrlKey || e.metaKey) && e.key === 's') {
                e.preventDefault();
                this.saveSkill();
            }
        });
    }

    async loadSkills() {
        try {
            const response = await fetch('/api/skills');
            const data = await response.json();

            if (data.success) {
                this.skills = data.skills;
                this.renderSkillList();
            } else {
                this.showError('Failed to load skills: ' + data.error);
            }
        } catch (error) {
            this.showError('Failed to load skills: ' + error.message);
        }
    }

    async loadTemplates() {
        try {
            const response = await fetch('/api/skills/templates');
            const data = await response.json();

            if (data.success) {
                this.templates = data.templates;
            }
        } catch (error) {
            console.error('Failed to load templates:', error);
        }
    }

    renderSkillList() {
        const listEl = document.getElementById('skill-list');

        if (this.skills.length === 0) {
            listEl.innerHTML = '<div class="loading">No skills yet. Create one!</div>';
            return;
        }

        listEl.innerHTML = this.skills.map(skill => `
            <div class="skill-item ${this.currentSkill?.meta.name === skill.name ? 'active' : ''}"
                 data-name="${skill.name}">
                <div class="skill-item-name">${skill.name}</div>
                <div class="skill-item-desc">${skill.description || 'No description'}</div>
            </div>
        `).join('');

        // Bind click events
        listEl.querySelectorAll('.skill-item').forEach(el => {
            el.addEventListener('click', () => {
                this.loadSkill(el.dataset.name);
            });
        });
    }

    async loadSkill(name) {
        try {
            const response = await fetch(`/api/skills/${name}`);
            const data = await response.json();

            if (data.success) {
                this.currentSkill = data.skill;
                this.showSkill(data.skill);
            } else {
                this.showError('Failed to load skill: ' + data.error);
            }
        } catch (error) {
            this.showError('Failed to load skill: ' + error.message);
        }
    }

    showSkill(skill) {
        document.getElementById('skill-name').value = skill.meta.name;
        this.editor.setValue(skill.content);
        this.updatePreview();
        this.showValidation(skill.validation_errors);
        this.renderSkillList();
    }

    async saveSkill() {
        if (!this.currentSkill) {
            this.showError('No skill to save');
            return;
        }

        const content = this.editor.getValue();
        const name = document.getElementById('skill-name').value;

        try {
            const response = await fetch(`/api/skills/${name}`, {
                method: 'PUT',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ content }),
            });

            const data = await response.json();

            if (data.success) {
                this.showSuccess('Skill saved successfully');
                await this.loadSkills();
            } else {
                this.showError('Failed to save: ' + data.error);
            }
        } catch (error) {
            this.showError('Failed to save: ' + error.message);
        }
    }

    async deleteSkill() {
        if (!this.currentSkill) {
            this.showError('No skill selected');
            return;
        }

        if (!confirm(`Delete skill "${this.currentSkill.meta.name}"?`)) {
            return;
        }

        try {
            const response = await fetch(`/api/skills/${this.currentSkill.meta.name}`, {
                method: 'DELETE',
            });

            const data = await response.json();

            if (data.success) {
                this.showSuccess('Skill deleted');
                this.currentSkill = null;
                this.editor.setValue('');
                await this.loadSkills();
            } else {
                this.showError('Failed to delete: ' + data.error);
            }
        } catch (error) {
            this.showError('Failed to delete: ' + error.message);
        }
    }

    async validateSkill() {
        const content = this.editor.getValue();

        try {
            const response = await fetch('/api/skills/_validate', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ content }),
            });

            const data = await response.json();

            if (data.success) {
                this.showValidation(data.errors);

                if (data.valid) {
                    this.showSuccess('Skill is valid!');
                } else {
                    this.showError('Validation failed');
                }
            }
        } catch (error) {
            this.showError('Validation failed: ' + error.message);
        }
    }

    showValidation(errors) {
        const resultsEl = document.getElementById('validation-results');

        if (!errors || errors.length === 0) {
            resultsEl.innerHTML = '<li class="validation-error info">No issues found</li>';
            return;
        }

        resultsEl.innerHTML = errors.map(error => `
            <li class="validation-error ${error.severity}">
                ${error.message}
                ${error.line ? `<div class="line">Line ${error.line}${error.column ? `, Column ${error.column}` : ''}</div>` : ''}
            </li>
        `).join('');
    }

    updatePreview() {
        const content = this.editor.getValue();
        const previewEl = document.getElementById('preview-content');

        try {
            previewEl.innerHTML = marked.parse(content);
        } catch (error) {
            previewEl.innerHTML = '<p style="color: var(--danger)">Preview error</p>';
        }
    }

    showNewSkillModal() {
        document.getElementById('new-skill-modal').classList.add('active');
        document.getElementById('new-skill-name').value = '';
        document.getElementById('new-skill-name').focus();
    }

    hideNewSkillModal() {
        document.getElementById('new-skill-modal').classList.remove('active');
    }

    async createNewSkill() {
        const name = document.getElementById('new-skill-name').value.trim();
        const template = document.getElementById('new-skill-template').value;

        if (!name) {
            this.showError('Skill name is required');
            return;
        }

        if (!/^[a-z0-9_-]+$/.test(name)) {
            this.showError('Skill name can only contain lowercase letters, numbers, hyphens, and underscores');
            return;
        }

        try {
            const response = await fetch('/api/skills', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ name, template: template || null }),
            });

            const data = await response.json();

            if (data.success) {
                this.hideNewSkillModal();
                await this.loadSkills();
                await this.loadSkill(name);
                this.showSuccess('Skill created!');
            } else {
                this.showError('Failed to create: ' + data.error);
            }
        } catch (error) {
            this.showError('Failed to create: ' + error.message);
        }
    }

    showSuccess(message) {
        // TODO: Implement toast notification
        alert('✓ ' + message);
    }

    showError(message) {
        // TODO: Implement toast notification
        alert('✗ ' + message);
    }
}

// Initialize on page load
document.addEventListener('DOMContentLoaded', () => {
    new SkillEditor();
});
