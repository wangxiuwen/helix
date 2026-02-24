import { createBrowserRouter, RouterProvider } from 'react-router-dom';

import Layout from './components/layout/Layout';
import CronJobs from './pages/CronJobs';
import Skills from './pages/Skills';
import Logs from './pages/Logs';
import Settings from './pages/Settings';
import WeChat from './pages/WeChat';
import ThemeManager from './components/common/ThemeManager';
import { useEffect } from 'react';
import { useConfigStore } from './stores/useConfigStore';
import { useTranslation } from 'react-i18next';

const router = createBrowserRouter([
  {
    path: '/',
    element: <Layout />,
    children: [
      {
        index: true,
        element: <WeChat />,
      },
      {
        path: 'cron-jobs',
        element: <CronJobs />,
      },
      {
        path: 'skills',
        element: <Skills />,
      },
      {
        path: 'logs',
        element: <Logs />,
      },
      {
        path: 'settings',
        element: <Settings />,
      },
    ],
  },
]);

function App() {
  const { config, loadConfig } = useConfigStore();
  const { i18n } = useTranslation();

  useEffect(() => {
    loadConfig();
  }, [loadConfig]);

  // Sync language from config
  useEffect(() => {
    if (config?.language) {
      i18n.changeLanguage(config.language);
      document.documentElement.dir = config.language === 'ar' ? 'rtl' : 'ltr';
    }
  }, [config?.language, i18n]);

  return (
    <>
      <ThemeManager />
      <RouterProvider router={router} />
    </>
  );
}

export default App;