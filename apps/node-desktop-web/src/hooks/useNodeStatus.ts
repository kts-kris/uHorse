import { useQuery } from '@tanstack/react-query';
import { desktopApi } from '../services/desktopApi';

export function useNodeStatus() {
  return useQuery({
    queryKey: ['desktop-runtime-status'],
    queryFn: desktopApi.getRuntimeStatus,
    refetchInterval: 5000,
  });
}
