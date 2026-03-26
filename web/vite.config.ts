import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

function isPackage(id: string, packageName: string): boolean {
  return (
    id.includes(`/node_modules/${packageName}/`) ||
    id.includes(`\\node_modules\\${packageName}\\`)
  );
}

function manualChunks(id: string): string | undefined {
  if (!id.includes('node_modules')) {
    return undefined;
  }

  if (isPackage(id, 'antd') || isPackage(id, '@ant-design/icons')) {
    return 'antd-vendor';
  }

  if (isPackage(id, '@tanstack/react-query') || isPackage(id, 'axios') || isPackage(id, 'dayjs')) {
    return 'shared-vendor';
  }

  if (isPackage(id, 'react') || isPackage(id, 'react-dom') || isPackage(id, 'scheduler')) {
    return 'react-vendor';
  }

  return 'vendor';
}

export default defineConfig({
  plugins: [react()],
  build: {
    rollupOptions: {
      output: {
        manualChunks,
      },
    },
  },
});
