import { sharedStrings } from '../../shared/i18n';
import type { Lang, Translations } from '../../shared/i18n';
import common from './common';
import cron from './cron';
import processes from './processes';
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
import terminal from './terminal';
import share from './share';
import dictation from './dictation';
import sessionSettings from './session-settings';
import sftp from './sftp';
import remoteProtocols from './remote-protocols';
import remoteBase from './remote-base';
import claude from './claude';
import claudePeer from './claude-peer';
import users from './users';
import usersDetail from './users-detail';
import usersAccess from './users-access';
import usersGrants from './users-grants';
import usersScope from './users-scope';
import scope from './scope';
import signup from './signup';
import sftpSniff from './sftp-sniff';

export type { Lang };
export { LANG_LABELS } from '../../shared/i18n';

const translations: Translations = {
  ...sharedStrings,
  ...common,
  ...cron,
  ...processes,
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
  ...terminal,
  ...share,
  ...dictation,
  ...sessionSettings,
  ...remoteBase,
  ...sftp,
  ...claude,
  ...claudePeer,
  ...remoteProtocols,
  ...users,
  ...usersDetail,
  ...usersAccess,
  ...usersGrants,
  ...usersScope,
  ...scope,
  ...signup,
  ...sftpSniff,
};

export default translations;
