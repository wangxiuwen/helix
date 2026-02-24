import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { Terminal } from 'lucide-react';

/**
 * Logo 组件 - helix 品牌
 */
export function NavLogo() {
    const { t } = useTranslation();

    return (
        <Link to="/" draggable="false" className="flex w-full min-w-0 items-center gap-2.5 text-xl font-semibold text-gray-900 dark:text-base-content">
            <div className="p-1.5 rounded-lg bg-gradient-to-br from-violet-500 to-blue-600">
                <Terminal size={20} className="text-white" />
            </div>
            <span className="hidden @[200px]/logo:inline text-nowrap">{t('common.app_name', 'helix')}</span>
        </Link>
    );
}
