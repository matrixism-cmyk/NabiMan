import React, { useState, useEffect, useRef } from 'react';
import { apiPost } from '../hooks/useApi';
import { useT } from '../i18n';
import { useSortable } from '../hooks/useSortable';

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
  const [uploading, setUploading] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const currentDir = pathInput.endsWith('/') ? pathInput : pathInput + '/';

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

  const handleUpload = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    setUploading(true);
    try {
      const text = await file.text();
      const token = sessionStorage.getItem('nabiman_token') || '';
      const filePath = currentDir + file.name;
      const res = await fetch('/api/files/upload', {
        method: 'POST',
        headers: {
          'Authorization': `Bearer ${token}`,
          'Content-Type': 'text/plain',
          'X-File-Path': filePath,
        },
        body: text,
      });
      const json = await res.json();
      setMsg(json.success ? t('files.uploaded') : json.message);
      if (json.success) browse(pathInput);
    } catch (err) {
      setMsg(err instanceof Error ? err.message : 'Upload failed');
    }
    setUploading(false);
    if (fileInputRef.current) fileInputRef.current.value = '';
  };

  const handleMkdir = async () => {
    const name = window.prompt(t('files.folderName'));
    if (!name) return;
    const res = await apiPost<string>('/api/files/mkdir', { path: currentDir + name });
    setMsg(res.success ? res.data || 'OK' : res.message);
    if (res.success) browse(pathInput);
  };

  const handleDelete = async (path: string) => {
    if (!window.confirm(t('files.confirmDelete'))) return;
    const res = await apiPost<string>('/api/files/delete', { path });
    setMsg(res.success ? res.data || 'OK' : res.message);
    if (res.success) browse(pathInput);
  };

  const handleDownload = (path: string) => {
    const token = sessionStorage.getItem('nabiman_token') || '';
    const url = `/api/files/download?path=${encodeURIComponent(path)}&token=${encodeURIComponent(token)}`;
    window.open(url, '_blank');
  };

  const handleChmod = async (path: string) => {
    const mode = window.prompt(t('files.chmodMode'), '755');
    if (!mode) return;
    const res = await apiPost<string>('/api/files/chmod', { path, mode });
    setMsg(res.success ? res.data || 'OK' : res.message);
    if (res.success) browse(pathInput);
  };

  const { sorted, toggle, indicator } = useSortable(files, 'name', 'asc');
  const S = (key: string, label: string) => (
    <th className="sortable" onClick={() => toggle(key)}>{label}{indicator(key)}</th>
  );

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>{t('files.title')}</h2>
      </div>
      <div className="filter-row">
        <input className="filter-input" value={pathInput} onChange={e => setPathInput(e.target.value)}
          onKeyDown={e => e.key === 'Enter' && browse(pathInput)} />
        <button className="btn btn-primary btn-sm" onClick={() => browse(pathInput)}>{t('files.go')}</button>
        <button className="btn btn-secondary btn-sm" onClick={() => fileInputRef.current?.click()} disabled={uploading}>
          {uploading ? t('files.uploading') : t('files.upload')}
        </button>
        <button className="btn btn-secondary btn-sm" onClick={handleMkdir}>{t('files.newFolder')}</button>
        <input ref={fileInputRef} type="file" style={{ display: 'none' }} onChange={handleUpload} />
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
          <thead>
            <tr>
              {S('name', t('common.name'))}
              {S('size', t('files.size'))}
              {S('permissions', t('files.perms'))}
              {S('modified', t('files.modified'))}
              <th>{t('common.actions')}</th>
            </tr>
          </thead>
          <tbody>
            {loading ? <tr><td colSpan={5} className="loading">{t('common.loading')}</td></tr> :
              sorted.map((f, i) => (
                <tr key={i}>
                  <td onClick={() => handleClick(f)} style={{ cursor: 'pointer' }}>
                    {f.is_dir ? '📁 ' : '📄 '}{f.name}
                  </td>
                  <td>{f.is_dir ? '-' : fmtSize(f.size)}</td>
                  <td><code>{f.permissions}</code></td>
                  <td>{f.modified}</td>
                  <td className="btn-group" onClick={e => e.stopPropagation()}>
                    {!f.is_dir && (
                      <button className="btn btn-sm btn-secondary" onClick={() => handleDownload(f.path)}>
                        {t('files.download')}
                      </button>
                    )}
                    <button className="btn btn-sm btn-secondary" onClick={() => handleChmod(f.path)}>
                      {t('files.chmod')}
                    </button>
                    <button className="btn btn-sm btn-danger" onClick={() => handleDelete(f.path)}>
                      {t('files.delete')}
                    </button>
                  </td>
                </tr>
              ))
            }
          </tbody>
        </table>
      )}
    </div>
  );
}
