export interface DesktopOverview {
  nodeName: string;
  connectionState: string;
  workspacePath: string;
  pendingApprovals: number;
  runningTasks: number;
  latestCheckpoint: string;
}

export interface WorkspaceItem {
  name: string;
  path: string;
  readOnly: boolean;
  gitRepo: boolean;
}

export interface VersionEntry {
  path: string;
  staged: string;
  unstaged: string;
}

export interface LogEntry {
  level: 'INFO' | 'WARN' | 'ERROR';
  message: string;
  timestamp: string;
}

export const overview: DesktopOverview = {
  nodeName: 'uHorse Node',
  connectionState: 'authenticated',
  workspacePath: '/workspace/project',
  pendingApprovals: 1,
  runningTasks: 2,
  latestCheckpoint: 'checkpoint: 保存 notes',
};

export const workspaces: WorkspaceItem[] = [
  {
    name: 'project',
    path: '/workspace/project',
    readOnly: false,
    gitRepo: true,
  },
];

export const versionEntries: VersionEntry[] = [
  { path: 'README.md', staged: 'modified', unstaged: 'unmodified' },
  { path: 'src/main.rs', staged: 'unmodified', unstaged: 'modified' },
  { path: 'notes.txt', staged: 'added', unstaged: 'unmodified' },
];

export const logs: LogEntry[] = [
  {
    level: 'INFO',
    message: 'Node connected to hub successfully',
    timestamp: '2026-03-22T10:00:00Z',
  },
  {
    level: 'WARN',
    message: 'Approval request waiting for reviewer',
    timestamp: '2026-03-22T10:05:00Z',
  },
  {
    level: 'ERROR',
    message: 'Workspace health check failed once and auto recovered',
    timestamp: '2026-03-22T10:08:00Z',
  },
];
