import { Clock, Blocks, Activity, Settings, MessageCircle } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useConfigStore } from '../../stores/useConfigStore';
import { isTauri, isLinux } from '../../utils/env';
import { NavLogo } from './NavLogo';
import { NavMenu } from './NavMenu';
import { NavSettings } from './NavSettings';
import type { NavItem } from './constants';

function Navbar() {
    const { t } = useTranslation();
    const { config, saveConfig } = useConfigStore();

    const navItems: NavItem[] = [
        { path: '/', label: t('nav.channels', '通道'), icon: MessageCircle, priority: 'high' },
        { path: '/skills', label: t('nav.skills', '技能'), icon: Blocks, priority: 'medium' },
        { path: '/cron-jobs', label: t('nav.cron_jobs', '定时任务'), icon: Clock, priority: 'medium' },
        { path: '/logs', label: t('nav.logs', '日志'), icon: Activity, priority: 'medium' },
        { path: '/settings', label: t('nav.settings', '设置'), icon: Settings, priority: 'high' },
    ];

    const toggleTheme = async (event: React.MouseEvent<HTMLButtonElement>) => {
        if (!config) return;
        const newTheme = config.theme === 'light' ? 'dark' : 'light';
        if ('startViewTransition' in document && !isLinux()) {
            const x = event.clientX;
            const y = event.clientY;
            const endRadius = Math.hypot(Math.max(x, window.innerWidth - x), Math.max(y, window.innerHeight - y));
            // @ts-ignore
            const transition = document.startViewTransition(async () => {
                saveConfig({ ...config, theme: newTheme, language: config.language }, true);
            });
            transition.ready.then(() => {
                const isDarkMode = newTheme === 'dark';
                const clipPath = isDarkMode
                    ? [`circle(${endRadius}px at ${x}px ${y}px)`, `circle(0px at ${x}px ${y}px)`]
                    : [`circle(0px at ${x}px ${y}px)`, `circle(${endRadius}px at ${x}px ${y}px)`];
                document.documentElement.animate({ clipPath }, {
                    duration: 500, easing: 'ease-in-out', fill: 'forwards',
                    pseudoElement: isDarkMode ? '::view-transition-old(root)' : '::view-transition-new(root)'
                });
            });
        } else {
            await saveConfig({ ...config, theme: newTheme, language: config.language }, true);
        }
    };

    const handleLanguageChange = async (langCode: string) => {
        if (!config) return;
        await saveConfig({ ...config, language: langCode, theme: config.theme }, true);
    };

    return (
        <nav style={{ position: 'sticky', top: 0, zIndex: 50 }} className="pt-9 transition-all duration-200 bg-[#FAFBFC] dark:bg-base-300">
            {isTauri() && (
                <div className="absolute top-9 left-0 right-0 h-16" style={{ zIndex: 5, backgroundColor: 'rgba(0,0,0,0.001)' }} data-tauri-drag-region />
            )}
            <div className="max-w-7xl mx-auto px-8 relative" style={{ zIndex: 10 }}>
                <div className="flex items-center h-16 gap-4">
                    <div className="@container/logo basis-[200px] shrink min-w-0">
                        <NavLogo />
                    </div>
                    <div className="flex-1 flex justify-center">
                        <NavMenu navItems={navItems} />
                    </div>
                    <NavSettings theme={(config?.theme as 'light' | 'dark') || 'light'} currentLanguage={config?.language || 'zh'} onThemeToggle={toggleTheme} onLanguageChange={handleLanguageChange} />
                </div>
            </div>
        </nav>
    );
}

export default Navbar;
