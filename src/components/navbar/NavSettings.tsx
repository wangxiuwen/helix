import { Sun, Moon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { LanguageDropdown, MoreDropdown } from './NavDropdowns';
import { LANGUAGES } from './constants';

interface NavSettingsProps {
    theme: 'light' | 'dark';
    currentLanguage: string;
    onThemeToggle: (event: React.MouseEvent<HTMLButtonElement>) => void;
    onLanguageChange: (langCode: string) => void;
}

export function NavSettings({
    theme,
    currentLanguage,
    onThemeToggle,
    onLanguageChange
}: NavSettingsProps) {
    const { t } = useTranslation();

    return (
        <>
            {/* Desktop buttons (≥ 480px) */}
            <div className="hidden min-[480px]:flex items-center gap-2">
                {/* Theme toggle */}
                <button
                    onClick={onThemeToggle}
                    className="w-10 h-10 rounded-full bg-gray-100 dark:bg-base-200 hover:bg-gray-200 dark:hover:bg-base-100 flex items-center justify-center transition-colors"
                    title={theme === 'light' ? t('nav.theme_to_dark') : t('nav.theme_to_light')}
                >
                    {theme === 'light' ? (
                        <Moon className="w-5 h-5 text-gray-700 dark:text-gray-300" />
                    ) : (
                        <Sun className="w-5 h-5 text-gray-700 dark:text-gray-300" />
                    )}
                </button>

                {/* Language dropdown */}
                <LanguageDropdown
                    currentLanguage={currentLanguage}
                    languages={LANGUAGES}
                    onLanguageChange={onLanguageChange}
                />
            </div>

            {/* Mobile menu (< 480px) */}
            <div className="min-[480px]:hidden">
                <MoreDropdown
                    theme={theme}
                    currentLanguage={currentLanguage}
                    languages={LANGUAGES}
                    onThemeToggle={onThemeToggle}
                    onLanguageChange={onLanguageChange}
                />
            </div>
        </>
    );
}
