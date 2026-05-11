export interface PortStatus {
  id: number;
  project_id: number;
  service: string;
  port: number;
  active: boolean;
  process: string | null;
  pid: number | null;
  created_at: string;
}

export type KillOutcome =
  | "terminated"
  | "killed"
  | "not_active"
  | "permission_denied";

export interface UnmanagedPort {
  port: number;
  process: string;
  pid: number;
}

export interface ProjectStatus {
  id: number;
  name: string;
  path: string | null;
  range_start: number;
  range_end: number;
  created_at: string;
  ports: PortStatus[];
}
