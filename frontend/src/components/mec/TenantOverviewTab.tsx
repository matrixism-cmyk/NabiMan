import React from 'react';
import { useT } from '../../i18n';
import { Tenant } from '../../types/mec';
import { Field, FieldGrid, Section, StatusBadge } from './common';

interface Props {
  tenant: Tenant;
}

export default function TenantOverviewTab({ tenant }: Props) {
  const { t } = useT();
  const status = 'state' in tenant.status ? tenant.status.state : 'unknown';
  return (
    <div>
      <Section title={t('mec.tenant.overview.basicInfo')}>
        <FieldGrid>
          <Field label={t('mec.tenant.col.id')} value={<code>{tenant.id}</code>} />
          <Field label={t('mec.tenant.overview.company')} value={tenant.display_name} />
          <Field label={t('mec.tenant.overview.taskName')} value={tenant.task_name || '-'} />
          <Field label={t('mec.tenant.overview.contactEmail')} value={tenant.contact_email || '-'} />
          <Field
            label={t('mec.tenant.overview.status')}
            value={<StatusBadge tone={status}>{status}</StatusBadge>}
          />
        </FieldGrid>
      </Section>

      <Section title={t('mec.tenant.overview.allocatedResources')} marginTop="20px">
        <FieldGrid>
          <Field label={t('mec.tenant.overview.namespace')} value={<code>{tenant.namespace}</code>} />
          <Field label="Rancher Project" value={tenant.rancher_project_id || '-'} />
          <Field label="Rancher User" value={tenant.rancher_user_id || '-'} />
          <Field label={t('mec.tenant.overview.allocationType')} value={tenant.allocation.type} />
          <Field
            label={t('mec.tenant.overview.node')}
            value={
              tenant.allocation.node ? (
                <StatusBadge tone="info">{tenant.allocation.node}</StatusBadge>
              ) : (
                t('mec.tenant.shared')
              )
            }
          />
          <Field
            label={t('mec.tenant.overview.gpuLabel')}
            value={tenant.allocation.gpu_label || '-'}
          />
        </FieldGrid>
      </Section>

      <Section title={t('mec.tenant.overview.history')} marginTop="20px">
        <FieldGrid>
          <Field
            label={t('mec.tenant.overview.created')}
            value={new Date(tenant.created_at).toLocaleString()}
          />
          <Field
            label={t('mec.tenant.overview.updated')}
            value={new Date(tenant.updated_at).toLocaleString()}
          />
        </FieldGrid>
      </Section>
    </div>
  );
}
