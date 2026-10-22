import React, { useEffect, useState } from 'react';
import { apiRequest } from '../../shared/api/useApi';
import type { AccessRules, MemberUser } from '../MembershipEditor';

const DAYS = ['월', '화', '수', '목', '금', '토', '일'];
const HOURS = Array.from({ length: 25 }, (_, h) => h);

/** datetime-local's value for an instant, in this browser's time. */
function local(iso?: string | null): string {
  if (!iso) return '';
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? '' : new Date(d.getTime() - d.getTimezoneOffset() * 60000).toISOString().slice(0, 16);
}

const privateAddress = (ip: string) => /^(10\.|127\.|192\.168\.|172\.(1[6-9]|2\d|3[01])\.|::1$|f[cd])/i.test(ip);

interface Props {
  user: MemberUser;
  busy: boolean;
  update: (body: object) => Promise<boolean>;
}

/** When and from where the account may be used, and how it signs in. */
export default function AccessRulesForm({ user, busy, update }: Props) {
  const saved: AccessRules = { expires_at: null, require_mfa: false, allowed_networks: [], allowed_hours: null, ...user.access };
  const [draft, setDraft] = useState<AccessRules>(saved);
  const [networks, setNetworks] = useState(saved.allowed_networks.join('\n'));
  const [mine, setMine] = useState('');
  useEffect(() => {
    apiRequest<string>('/api/users/my-address').then(r => { if (r.success && r.data) setMine(r.data); }).catch(() => undefined);
  }, []);

  const next: AccessRules = { ...draft, allowed_networks: networks.split(/[\s,]+/).map(s => s.trim()).filter(Boolean) };
  const changed = JSON.stringify(next) !== JSON.stringify(saved);
  const hours = draft.allowed_hours;
  const setHours = (h: AccessRules['allowed_hours']) => setDraft(d => ({ ...d, allowed_hours: h }));
  const expireIn = (days: number) => setDraft(d => ({ ...d, expires_at: new Date(Date.now() + days * 86400000).toISOString() }));

  return (
    <div className="users-sections">
      <section className="users-card">
        <h4>이용 기간</h4>
        <div className="users-row">
          <label>이용 만료일 (이 브라우저 시간)
            <input type="datetime-local" className="filter-input" value={local(draft.expires_at)}
              onChange={e => setDraft(d => ({ ...d, expires_at: e.target.value ? new Date(e.target.value).toISOString() : null }))} />
          </label>
          <div className="btn-group">
            {[30, 90, 365].map(n => <button key={n} type="button" className="btn btn-secondary btn-sm" onClick={() => expireIn(n)}>+{n === 365 ? '1년' : `${n}일`}</button>)}
            <button type="button" className="btn btn-secondary btn-sm" onClick={() => setDraft(d => ({ ...d, expires_at: null }))}>기한 없음</button>
          </div>
        </div>
        <p className="text-secondary">{draft.expires_at
          ? `${new Date(draft.expires_at).toLocaleString()} 이후로는 로그인할 수 없고, 열려 있던 로그인도 끝납니다.`
          : '기한 없이 쓸 수 있습니다.'}</p>
      </section>

      <section className="users-card">
        <h4>2단계 인증</h4>
        <label className={`users-grant ${draft.require_mfa ? 'is-on' : ''}`}>
          <input type="checkbox" checked={draft.require_mfa} onChange={e => setDraft(d => ({ ...d, require_mfa: e.target.checked }))} />
          <span><strong>2단계 인증 필수</strong>
            <small>{user.totp_enabled ? '이 회원은 이미 2단계 인증을 쓰고 있습니다.'
              : '아직 등록하지 않았습니다. 켜면 다음 로그인 때 등록 화면부터 나오고, 등록해야 쓸 수 있습니다.'}</small></span>
        </label>
      </section>

      <section className="users-card">
        <h4>접속 허용 주소</h4>
        <label>한 줄에 하나씩 — 주소(203.0.113.7) 또는 대역(203.0.113.0/24). 비우면 어디서나 됩니다.
          <textarea className="filter-input users-networks" rows={4} value={networks} spellCheck={false}
            onChange={e => setNetworks(e.target.value)} placeholder={'203.0.113.7\n198.51.100.0/24'} />
        </label>
        {mine && (
          <p className="text-secondary">
            이 서버가 보는 지금 내 접속 주소: <code>{mine}</code>{' '}
            <button type="button" className="btn btn-secondary btn-sm"
              onClick={() => setNetworks(n => (n.trim() ? `${n.trim()}\n${mine}` : mine))}>추가</button>
            {privateAddress(mine) && <span className="users-warn"> 내부 주소로 보입니다. 프록시 뒤라서 실제 주소를 모르는 상태일 수 있으니, 이 주소로 제한하기 전에 확인하세요.</span>}
          </p>
        )}
      </section>

      <section className="users-card">
        <h4>접속 허용 시간</h4>
        <label className={`users-grant ${hours ? 'is-on' : ''}`}>
          <input type="checkbox" checked={Boolean(hours)}
            onChange={e => setHours(e.target.checked ? { start: 9, end: 18, days: [0, 1, 2, 3, 4] } : null)} />
          <span><strong>정해진 시간에만</strong><small>한국 시간 기준입니다. 시간이 지나면 열려 있던 로그인과 터미널도 끊깁니다.</small></span>
        </label>
        {hours && (
          <div className="users-hours">
            <label>시작
              <select className="select-input" value={hours.start} onChange={e => setHours({ ...hours, start: Number(e.target.value) })}>
                {HOURS.slice(0, 24).map(h => <option key={h} value={h}>{String(h).padStart(2, '0')}:00</option>)}
              </select>
            </label>
            <label>끝
              <select className="select-input" value={hours.end} onChange={e => setHours({ ...hours, end: Number(e.target.value) })}>
                {HOURS.slice(1).map(h => <option key={h} value={h}>{String(h).padStart(2, '0')}:00</option>)}
              </select>
            </label>
            <div className="users-days" role="group" aria-label="허용 요일">
              {DAYS.map((d, i) => (
                <button key={d} type="button" aria-pressed={hours.days.includes(i)} className={hours.days.includes(i) ? 'is-on' : ''}
                  onClick={() => setHours({ ...hours, days: hours.days.includes(i) ? hours.days.filter(x => x !== i) : [...hours.days, i].sort() })}>{d}</button>
              ))}
            </div>
            <small className="text-secondary">{hours.end <= hours.start ? '끝이 시작보다 이르면 자정을 넘깁니다. ' : ''}요일을 하나도 고르지 않으면 매일입니다.</small>
          </div>
        )}
      </section>

      <div className="btn-group users-save">
        <button type="button" className="btn btn-primary btn-sm" disabled={busy || !changed}
          onClick={() => update({ access: next })}>접속 제한 저장</button>
        <button type="button" className="btn btn-secondary btn-sm" disabled={!changed}
          onClick={() => { setDraft(saved); setNetworks(saved.allowed_networks.join('\n')); }}>되돌리기</button>
      </div>
    </div>
  );
}
