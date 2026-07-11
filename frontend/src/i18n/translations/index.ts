import common from './common';
import monitoring from './monitoring';
import management from './management';
import security from './security';
import mec from './mec';
import mecLive from './mec-live';
import mecDashboard from './mec-dashboard';
import mecTenants from './mec-tenants';
import mecNodes from './mec-nodes';
import mecGpuStorage from './mec-gpu-storage';
import mecFirewall from './mec-firewall';
import mecOps from './mec-ops';
import mecWizard from './mec-wizard';
import noc from './noc';

export type Lang = 'ko' | 'en' | 'ja';

export const LANG_LABELS: Record<Lang, string> = {
  ko: '한국어',
  en: 'English',
  ja: '日本語',
};

const translations: Record<string, Record<Lang, string>> = {
  ...common,
  ...monitoring,
  ...management,
  ...security,
  ...mec,
  ...mecLive,
  ...mecDashboard,
  ...mecTenants,
  ...mecNodes,
  ...mecGpuStorage,
  ...mecFirewall,
  ...mecOps,
  ...mecWizard,
  ...noc,
};

export default translations;
