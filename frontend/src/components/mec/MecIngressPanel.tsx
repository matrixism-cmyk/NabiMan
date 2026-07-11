import React, { useState } from 'react';
import { useT } from '../../i18n';
import { useMecList, mecPost, mecDelete } from '../../hooks/mec/useMecApi';
import {
  ErrorBanner,
  PanelLayout,
  SortableTable,
  Toolbar,
} from './common';

interface IngressPathRow {
  host?: string | null;
  path: string;
  path_type: string;
  backend_service: string;
  backend_port: number;
}

interface IngressRow {
  namespace: string;
  name: string;
  class?: string | null;
  hosts: string[];
  paths: IngressPathRow[];
  age_seconds: number;
}

function humanize(secs: number): string {
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m`;
  if (secs < 86400) return `${Math.floor(secs / 3600)}h`;
  return `${Math.floor(secs / 86400)}d`;
}

export default function MecIngressPanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useMecList<IngressRow>(
    '/api/mec/v1/network/ingresses',
    60_000,
  );
  const [showAdd, setShowAdd] = useState(false);
  const [form, setForm] = useState({
    namespace: '',
    name: '',
    host: '',
    class: 'nginx',
    backend_service: '',
    backend_port: 80,
    path: '/',
    tls_secret_name: '',
  });
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [filter, setFilter] = useState('');

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setErr(null);
    try {
      const res = await mecPost('/api/mec/v1/network/ingresses', {
        namespace: form.namespace,
        name: form.name,
        host: form.host,
        class: form.class,
        backend_service: form.backend_service,
        backend_port: form.backend_port,
        path: form.path,
        path_type: 'Prefix',
        tls_secret_name: form.tls_secret_name || null,
      });
      if (res.error) setErr(res.error.message);
      else {
        setShowAdd(false);
        await refetch();
      }
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  };

  const remove = async (ns: string, name: string) => {
    const typed = window.prompt(
      t('mec.ingress.deletePrompt', { ns, name }),
      '',
    );
    if (typed !== name) {
      if (typed !== null) setErr(t('mec.ingress.deleteMismatch'));
      return;
    }
    try {
      await mecDelete(
        `/api/mec/v1/network/ingresses/${encodeURIComponent(ns)}/${encodeURIComponent(name)}`,
        name,
      );
      await refetch();
    } catch (e) {
      setErr(String(e));
    }
  };

  return (
    <PanelLayout
      title={t('mec.ingress.title')}
      subtitle={t('mec.ingress.subtitle', { count: data?.length || 0 })}
      actions={
        <>
          <button className="btn btn-secondary" onClick={refetch}>
            {t('mec.action.refresh')}
          </button>
          <button className="btn btn-primary" onClick={() => setShowAdd((v) => !v)}>
            {showAdd ? t('mec.ingress.cancel') : t('mec.ingress.add')}
          </button>
        </>
      }
    >
      <ErrorBanner error={error || err || undefined} />

      {showAdd && (
        <form
          onSubmit={submit}
          style={{
            background: 'white',
            border: '1px solid #e5e7eb',
            padding: '14px',
            borderRadius: '8px',
            marginBottom: '12px',
          }}
        >
          <h3 style={{ marginTop: 0 }}>{t('mec.ingress.formTitle')}</h3>
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(auto-fit, minmax(180px, 1fr))',
              gap: '8px',
            }}
          >
            <label>
              Namespace
              <input
                required
                value={form.namespace}
                onChange={(e) => setForm({ ...form, namespace: e.target.value })}
              />
            </label>
            <label>
              {t('mec.ingress.name')}
              <input
                required
                value={form.name}
                onChange={(e) => setForm({ ...form, name: e.target.value })}
              />
            </label>
            <label>
              Host
              <input
                required
                value={form.host}
                onChange={(e) => setForm({ ...form, host: e.target.value })}
                placeholder="app.jcia.mec.local"
              />
            </label>
            <label>
              IngressClass
              <input
                value={form.class}
                onChange={(e) => setForm({ ...form, class: e.target.value })}
              />
            </label>
            <label>
              Backend Service
              <input
                required
                value={form.backend_service}
                onChange={(e) =>
                  setForm({ ...form, backend_service: e.target.value })
                }
              />
            </label>
            <label>
              Backend Port
              <input
                type="number"
                value={form.backend_port}
                onChange={(e) =>
                  setForm({
                    ...form,
                    backend_port: parseInt(e.target.value, 10) || 80,
                  })
                }
              />
            </label>
            <label>
              Path
              <input
                value={form.path}
                onChange={(e) => setForm({ ...form, path: e.target.value })}
              />
            </label>
            <label>
              {t('mec.ingress.tlsSecret')}
              <input
                value={form.tls_secret_name}
                onChange={(e) =>
                  setForm({ ...form, tls_secret_name: e.target.value })
                }
              />
            </label>
          </div>
          <div style={{ marginTop: '12px' }}>
            <button type="submit" className="btn btn-primary" disabled={busy}>
              {busy ? t('mec.ingress.creating') : t('mec.ingress.create')}
            </button>
          </div>
        </form>
      )}

      <Toolbar>
        <input
          placeholder={t('mec.ingress.filter')}
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          style={{ padding: '6px 10px', minWidth: '240px' }}
        />
      </Toolbar>

      {loading && !data && <div style={{ color: '#6b7280' }}>{t('mec.state.loading')}</div>}

      <SortableTable<IngressRow>
        data={data || []}
        filter={filter}
        defaultSortKey="namespace"
        rowKey={(i) => `${i.namespace}/${i.name}`}
        emptyMessage={t('mec.ingress.noRules')}
        columns={[
          {
            key: 'namespace',
            header: 'Namespace',
            accessor: (i) => i.namespace,
            sortable: true,
          },
          {
            key: 'name',
            header: t('mec.ingress.name'),
            accessor: (i) => i.name,
            render: (i) => <code>{i.name}</code>,
            sortable: true,
          },
          {
            key: 'class',
            header: 'Class',
            accessor: (i) => i.class || '',
            render: (i) =>
              i.class || <span style={{ color: '#d1d5db' }}>-</span>,
            sortable: true,
          },
          {
            key: 'host',
            header: 'Host',
            accessor: (i) => i.hosts.join(','),
            render: (i) => i.hosts.join(', ') || '-',
            sortable: true,
          },
          {
            key: 'backend',
            header: 'Backend',
            accessor: (i) => i.paths.map((p) => p.backend_service).join(','),
            render: (i) =>
              i.paths
                .map((p) => `${p.backend_service}:${p.backend_port}${p.path}`)
                .join(', ') || '-',
            sortable: true,
          },
          {
            key: 'age',
            header: 'Age',
            accessor: (i) => i.age_seconds,
            render: (i) => humanize(i.age_seconds),
            width: '80px',
            align: 'right',
            sortable: true,
          },
          {
            key: 'actions',
            header: '',
            accessor: () => '',
            sortable: false,
            width: '80px',
            render: (i) => (
              <button
                className="btn btn-danger btn-small"
                onClick={() => remove(i.namespace, i.name)}
              >
                {t('mec.ingress.delete')}
              </button>
            ),
          },
        ]}
      />
    </PanelLayout>
  );
}
