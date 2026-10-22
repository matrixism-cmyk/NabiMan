import React, { useState } from 'react';
import type { MemberUser } from '../MembershipEditor';
import { DEFAULT_GRANTS, MemberGrants } from '../remote/member';

type Flag = { [K in keyof MemberGrants]: MemberGrants[K] extends boolean ? K : never }[keyof MemberGrants];
type Limit = { [K in keyof MemberGrants]: MemberGrants[K] extends number ? K : never }[keyof MemberGrants];

interface Item<K> { key: K; label: string; hint: string }

const SHARING: Item<Flag>[] = [
  { key: 'share_sessions', label: '세션 공유', hint: '터미널 세션을 계정 없는 사람에게 링크로 공유합니다.' },
  { key: 'share_files', label: '파일 공유', hint: '서버 파일을 링크로 공유하고, 세션 링크에 파일 패널을 붙입니다.' },
  { key: 'share_require_password', label: '링크에 비밀번호 필수', hint: '비밀번호 없는 링크는 만들 수 없습니다.' },
  { key: 'share_view_only', label: '보기 전용 링크만', hint: '방문자는 입력할 수 없고, 파일은 보기·내려받기만 합니다.' },
];
const FEATURES: Item<Flag>[] = [
  { key: 'dictation', label: '받아쓰기', hint: '마이크로 말해서 터미널에 입력합니다. 관리자의 음성 서비스 사용량이 듭니다.' },
  { key: 'register_servers', label: '서버 직접 등록', hint: '끄면 관리자가 지정한 서버만 씁니다. 이미 등록한 서버의 정보도 바꿀 수 없습니다.' },
  { key: 'assigned_file_write', label: '지정 서버 파일 쓰기', hint: '지정 서버에 올리기·지우기·이름 바꾸기·권한 바꾸기. 끄면 보기·내려받기만 합니다.' },
  { key: 'copy_between_servers', label: '서버 간 복사', hint: '한 서버의 파일을 다른 서버로 옮겨 복사합니다. 이 서버의 대역폭을 씁니다.' },
  { key: 'keep_sessions', label: '세션 유지', hint: '접속이 끊겨도 세션을 시간 제한 없이 살려 둡니다. 끄면 1시간 쓰지 않으면 닫힙니다.' },
];
const LIMITS: (Item<Limit> & { unit: string; max: number })[] = [
  { key: 'share_max_days', label: '공유 링크 최대 기간', unit: '일', max: 90, hint: '정하면 영구 링크는 만들 수 없습니다.' },
  { key: 'max_sessions', label: '동시 세션', unit: '개', max: 100, hint: '한꺼번에 열어 둘 수 있는 터미널 세션 수.' },
  { key: 'max_idle_minutes', label: '최대 유휴 시간', unit: '분', max: 43200, hint: '이만큼 쓰지 않은 세션은 닫힙니다. 정하면 세션 유지는 꺼집니다.' },
  { key: 'max_servers', label: '직접 등록 서버', unit: '대', max: 1000, hint: '직접 등록할 수 있는 서버 수.' },
  { key: 'max_upload_mb', label: '업로드 한 파일', unit: 'MB', max: 1048576, hint: '한 번에 올릴 수 있는 파일 크기.' },
];

interface Props {
  user: MemberUser;
  busy: boolean;
  update: (body: object) => Promise<boolean>;
}

/** What a remote-only member may do beyond working on their servers. Limits of 0 mean none. */
export default function RemoteGrantsForm({ user, busy, update }: Props) {
  const saved: MemberGrants = { ...DEFAULT_GRANTS, ...user.remote_permissions };
  const [draft, setDraft] = useState<MemberGrants>(saved);
  const changed = JSON.stringify(draft) !== JSON.stringify(saved);
  const set = <K extends keyof MemberGrants>(key: K, value: MemberGrants[K]) => setDraft(d => ({ ...d, [key]: value }));

  const flags = (title: string, items: Item<Flag>[], note?: string) => (
    <section className="users-card">
      <h4>{title}</h4>
      {note && <p className="text-secondary">{note}</p>}
      <div className="users-grants">
        {items.map(item => {
          const off = (item.key === 'share_require_password' || item.key === 'share_view_only')
            && !draft.share_sessions && !draft.share_files;
          return (
            <label key={item.key} className={`users-grant ${draft[item.key] ? 'is-on' : ''} ${off ? 'is-dim' : ''}`}>
              <input type="checkbox" checked={draft[item.key]} onChange={e => set(item.key, e.target.checked)} />
              <span><strong>{item.label}</strong><small>{item.hint}</small></span>
            </label>
          );
        })}
      </div>
    </section>
  );

  return (
    <div className="users-sections">
      {flags('공유', SHARING, '켜지 않으면 공유 링크를 만들 수 없습니다. 권한을 끄면 이미 나간 링크도 바로 막힙니다.')}
      {flags('기능', FEATURES)}
      <section className="users-card">
        <h4>한도 <span className="text-secondary">(0 = 제한 없음)</span></h4>
        <div className="users-limits">
          {LIMITS.map(item => (
            <label key={item.key} className="users-limit">
              <span>{item.label}</span>
              <span className="users-limit-input">
                <input type="number" min={0} max={item.max} className="filter-input" value={draft[item.key]}
                  onChange={e => set(item.key, Math.max(0, Math.min(item.max, Math.floor(Number(e.target.value) || 0))))} />
                <em>{item.unit}</em>
              </span>
              <small>{item.hint}</small>
            </label>
          ))}
        </div>
      </section>
      <div className="btn-group users-save">
        <button type="button" className="btn btn-primary btn-sm" disabled={busy || !changed}
          onClick={() => update({ remote_permissions: draft })}>원격 권한 저장</button>
        <button type="button" className="btn btn-secondary btn-sm" disabled={!changed} onClick={() => setDraft(saved)}>되돌리기</button>
        <button type="button" className="btn btn-secondary btn-sm" onClick={() => setDraft(DEFAULT_GRANTS)}>기본값으로</button>
      </div>
      <p className="text-secondary">저장하면 바로 적용됩니다. 회원 화면의 버튼은 회원이 새로 고칠 때 바뀝니다.</p>
    </div>
  );
}
