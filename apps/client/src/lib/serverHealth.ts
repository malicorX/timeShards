export type ServerHealth = {
  status: string;
  version: string;
  demo_seeding_enabled?: boolean;
  default_password_login_blocked?: boolean;
  hardware_adapter?: string;
  hardware_adapter_configured?: string | null;
  hardware_tcp_listen?: string | null;
};
