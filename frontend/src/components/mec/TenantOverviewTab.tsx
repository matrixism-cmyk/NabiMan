import React from 'react';
import { Tenant } from '../../types/mec';
import { Field, FieldGrid, Section, StatusBadge } from './common';

interface Props {
  tenant: Tenant;
}

export default function TenantOverviewTab({ tenant }: Props) {
  const status = 'state' in tenant.status ? tenant.status.state : 'unknown';
  return (
    <div>
      <Section title="기본 정보">
        <FieldGrid>
          <Field label="테넌트 ID" value={<code>{tenant.id}</code>} />
          <Field label="기업명" value={tenant.display_name} />
          <Field label="과제명" value={tenant.task_name || '-'} />
          <Field label="담당자 이메일" value={tenant.contact_email || '-'} />
          <Field
            label="상태"
            value={<StatusBadge tone={status}>{status}</StatusBadge>}
          />
        </FieldGrid>
      </Section>

      <Section title="할당 리소스" marginTop="20px">
        <FieldGrid>
          <Field label="네임스페이스" value={<code>{tenant.namespace}</code>} />
          <Field label="Rancher Project" value={tenant.rancher_project_id || '-'} />
          <Field label="Rancher User" value={tenant.rancher_user_id || '-'} />
          <Field label="할당 유형" value={tenant.allocation.type} />
          <Field
            label="노드"
            value={
              tenant.allocation.node ? (
                <StatusBadge tone="info">{tenant.allocation.node}</StatusBadge>
              ) : (
                '공유'
              )
            }
          />
          <Field
            label="GPU 라벨"
            value={tenant.allocation.gpu_label || '-'}
          />
        </FieldGrid>
      </Section>

      <Section title="이력" marginTop="20px">
        <FieldGrid>
          <Field
            label="생성"
            value={new Date(tenant.created_at).toLocaleString()}
          />
          <Field
            label="최종 갱신"
            value={new Date(tenant.updated_at).toLocaleString()}
          />
        </FieldGrid>
      </Section>
    </div>
  );
}
