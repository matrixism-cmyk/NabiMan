import React, { useState, useEffect } from 'react';
import { apiPost } from '../hooks/useApi';
import { useT } from '../i18n';

interface FileEntry { name: string; path: string; is_dir: boolean; size: number; modified: string; permissions: string; }
interface FileContent { path: string; content: string; size: number; }

function fmtSize(b: number): string {
  if (b === 0) return '-';
  if (b < 1024) return b + ' B';
  if (b < 1048576) return (b / 1024).toFixed(1) + ' KB';
  if (b < 1073741824) return (b / 1048576).toFixed(1) + ' MB';
  return (b / 1073741824).toFixed(1) + ' GB';
}

export default function FileManagerPanel() {
  const { t } = useT();
  const [files, setFiles] = useState<FileEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [editing, setEditing] = useState<FileContent | null>(null);
  const [editContent, setEditContent] = useState('');
  const [msg, setMsg] = useState('');
  const [pathInput, setPathInput] = useState('/');

  const browse = async (dir: string) => {
    setLoading(true);
    setEditing(null);
    const res = await fetch(`/api/files/browse?path=${encodeURIComponent(dir)}`, {
      headers: { 'Authorization': `Bearer ${sessionStorage.getItem('nabiman_token') || ''}`, 'Content-Type': 'application/json' },
    });
    const json = await res.json();
    if (json.success) { setFiles(json.data); setPathInput(dir); }
    else { setMsg(json.message); }
    setLoading(false);
  };

  useEffect(() => { browse('/'); }, []);

  const openFile = async (path: string) => {
    const res = await apiPost<FileContent>('/api/files/read', { path });
    if (res.success && res.data) { setEditing(res.data); setEditContent(res.data.content); }
    else { setMsg(res.message); }
  };

  const saveFile = async () => {
    if (!editing) return;
    const res = await apiPost<string>('/api/files/write', { path: editing.path, content: editContent });
    setMsg(res.success ? t('files.saved') : res.message);
  };

  const handleClick = (f: FileEntry) => {
    if (f.is_dir) browse(f.path);
    else openFile(f.path);
  };

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>{t('files.title')}</h2>
      </div>
      <div className="filter-row">
        <input className="filter-input" value={pathInput} onChange={e => setPathInput(e.target.value)}
          onKeyDown={e => e.key === 'Enter' && browse(pathInput)} />
        <button className="btn btn-primary btn-sm" onClick={() => browse(pathInput)}>{t('files.go')}</button>
      </div>
      {msg && <div className="message" onClick={() => setMsg('')}>{msg}</div>}
      {editing ? (
        <div>
          <div className="filter-row">
            <code>{editing.path}</code>
            <button className="btn btn-primary btn-sm" onClick={saveFile}>{t('common.save')}</button>
            <button className="btn btn-secondary btn-sm" onClick={() => setEditing(null)}>{t('common.close')}</button>
          </div>
          <textarea className="file-editor" value={editContent} onChange={e => setEditContent(e.target.value)} />
        </div>
      ) : (
        <table className="data-table">
          <thead><tr><th>{t('common.name')}</th><th>{t('files.size')}</th><th>{t('files.perms')}</th><th>{t('files.modified')}</th></tr></thead>
          <tbody>
            {loading ? <tr><td colSpan={4} className="loading">{t('common.loading')}</td></tr> :
              files.map((f, i) => (
                <tr key={i} onClick={() => handleClick(f)} style={{ cursor: 'pointer' }}>
                  <td>{f.is_dir ? '📁 ' : '📄 '}{f.name}</td>
                  <td>{f.is_dir ? '-' : fmtSize(f.size)}</td>
                  <td><code>{f.permissions}</code></td>
                  <td>{f.modified}</td>
                </tr>
              ))
            }
          </tbody>
        </table>
      )}
    </div>
  );
}
