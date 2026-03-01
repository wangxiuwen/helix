import React, { useState, useRef, useMemo, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { X, Upload, Check, Dices, ImageIcon, Sparkles } from 'lucide-react';
import { createAvatar } from '@dicebear/core';
import { micah, notionists, bottts, adventurer, funEmoji, lorelei } from '@dicebear/collection';

const DEFAULT_PRESETS = Array.from({ length: 11 }).map((_, i) => ({
    id: `preset-micah-${i}`,
    title: `预设头像 ${i + 1}`,
    url: createAvatar(micah, { seed: `helix-preset-${i}`, size: 128 }).toDataUri()
}));

export const BUILT_IN_AVATARS = [
    { id: 'helix-default', title: 'Helix', url: '/helix-logo.png' },
    ...DEFAULT_PRESETS
];

const AVATAR_STYLES = [
    { id: 'notionists', title: '简笔风格 (Notion)', style: notionists, bgAllowed: false },
    { id: 'lorelei', title: '水彩手绘 (Lorelei)', style: lorelei, bgAllowed: true },
    { id: 'micah', title: '质感现代 (Micah)', style: micah, bgAllowed: true },
    { id: 'bottts', title: '机甲潮玩 (Bottts)', style: bottts, bgAllowed: false },
    { id: 'funEmoji', title: '趣味涂鸦 (Emoji)', style: funEmoji, bgAllowed: false },
    { id: 'adventurer', title: '像素冒险 (Adventurer)', style: adventurer, bgAllowed: true }
];

const BG_COLORS = [
    { id: 'transparent', value: 'transparent', label: '透明' },
    { id: 'yellow', value: 'fde047', label: '明黄' },
    { id: 'green', value: '86efac', label: '浅绿' },
    { id: 'blue', value: '93c5fd', label: '天蓝' },
    { id: 'pink', value: 'f9a8d4', label: '粉红' },
    { id: 'purple', value: 'd8b4fe', label: '淡紫' },
    { id: 'gray', value: 'e5e7eb', label: '灰白' }
];

interface AvatarPickerProps {
    isOpen: boolean;
    onClose: () => void;
    onSelect: (url: string) => void;
    currentAvatarUrl?: string;
    title?: string;
}

export function AvatarPicker({ isOpen, onClose, onSelect, currentAvatarUrl, title }: AvatarPickerProps) {
    const { t } = useTranslation();
    const fileInputRef = useRef<HTMLInputElement>(null);

    const [activeTab, setActiveTab] = useState<'generator' | 'upload'>('generator');

    // Generator State
    const [genSeed, setGenSeed] = useState<string>('');
    const [genStyleIdx, setGenStyleIdx] = useState<number>(2);
    const [genBgIdx, setGenBgIdx] = useState<number>(0);

    // Upload State
    const [uploadPreviewUrl, setUploadPreviewUrl] = useState<string | null>(null);

    // Initialize random seed once
    useEffect(() => {
        if (isOpen && !genSeed) {
            setGenSeed(Math.random().toString(36).substring(2, 9));
        }
    }, [isOpen, genSeed]);

    // Track active selection for current mode
    const [finalUrl, setFinalUrl] = useState<string | null>(currentAvatarUrl || null);

    const generatedAvatarDataUri = useMemo(() => {
        if (!genSeed) return '';
        const currentStyle = AVATAR_STYLES[genStyleIdx];
        const currentBg = BG_COLORS[genBgIdx];

        const options: any = { seed: genSeed, size: 256 };
        if (currentStyle.bgAllowed && currentBg.value !== 'transparent') {
            options.backgroundColor = [currentBg.value];
        }

        const avatar = createAvatar(currentStyle.style as any, options);
        return avatar.toDataUri();
    }, [genSeed, genStyleIdx, genBgIdx]);

    useEffect(() => {
        if (activeTab === 'generator' && generatedAvatarDataUri) {
            setFinalUrl(generatedAvatarDataUri);
        } else if (activeTab === 'upload' && uploadPreviewUrl) {
            setFinalUrl(uploadPreviewUrl);
        }
    }, [activeTab, generatedAvatarDataUri, uploadPreviewUrl]);

    if (!isOpen) return null;

    const randomize = () => setGenSeed(Math.random().toString(36).substring(2, 9));

    const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        const file = e.target.files?.[0];
        if (file) {
            const reader = new FileReader();
            reader.onload = (event) => {
                const base64Str = event.target?.result as string;
                setUploadPreviewUrl(base64Str);
                setFinalUrl(base64Str);
            };
            reader.readAsDataURL(file);
        }
    };

    const handleConfirm = () => {
        if (finalUrl) {
            onSelect(finalUrl);
        }
        onClose();
    };

    return (
        <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/40 backdrop-blur-sm" style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}>
            <div className="bg-[#fcfcfc] dark:bg-[#2A2A2A] rounded-2xl shadow-xl w-[480px] overflow-hidden flex flex-col max-h-[85vh]">
                <div className="flex justify-between items-center px-5 py-4 border-b border-gray-100 dark:border-white/5 bg-white dark:bg-[#333333]">
                    <h3 className="font-semibold text-gray-800 dark:text-gray-200">
                        {title || t('avatar.picker_title', '个性化头像定制')}
                    </h3>
                    <button
                        onClick={onClose}
                        className="text-gray-400 hover:text-gray-600 dark:hover:text-gray-200 transition-colors w-7 h-7 flex items-center justify-center rounded-md hover:bg-black/5 dark:hover:bg-white/10"
                    >
                        <X size={16} />
                    </button>
                </div>

                <div className="flex border-b border-gray-100 dark:border-white/5 px-2 bg-gray-50/50 dark:bg-[#2c2c2c]/50">
                    <button
                        className={`flex items-center gap-2 px-4 py-3 text-sm font-medium border-b-2 transition-colors ${activeTab === 'generator' ? 'border-[#07c160] text-[#07c160]' : 'border-transparent text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200'}`}
                        onClick={() => setActiveTab('generator')}
                    >
                        <Sparkles size={16} /> {t('avatar.tab_procedural', '✨ 随机简笔画')}
                    </button>
                    <button
                        className={`flex items-center gap-2 px-4 py-3 text-sm font-medium border-b-2 transition-colors ${activeTab === 'upload' ? 'border-[#07c160] text-[#07c160]' : 'border-transparent text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200'}`}
                        onClick={() => setActiveTab('upload')}
                    >
                        <ImageIcon size={16} /> {t('avatar.tab_local', '📁 本地与预设')}
                    </button>
                </div>

                <div className="flex-1 overflow-y-auto p-5 relative">
                    {/* Generative Tab */}
                    {activeTab === 'generator' && (
                        <div className="flex flex-col gap-6 animate-in fade-in duration-200">
                            {/* Main Preview */}
                            <div className="flex flex-col items-center justify-center">
                                <div className="relative w-32 h-32 rounded-2xl overflow-hidden shadow-sm border border-black/5 dark:border-white/10 bg-white dark:bg-black/20 group">
                                    {generatedAvatarDataUri ? (
                                        <img src={generatedAvatarDataUri} alt="Generated Avatar" className="w-full h-full object-cover transition-transform group-hover:scale-105" />
                                    ) : (
                                        <div className="w-full h-full flex items-center justify-center text-gray-300"><Sparkles size={32} /></div>
                                    )}
                                </div>
                                <div className="mt-4 flex items-center gap-3">
                                    <button
                                        onClick={randomize}
                                        className="flex items-center gap-2 px-6 py-2 bg-gradient-to-r from-[#07c160] to-[#06ad56] hover:opacity-90 text-white rounded-xl shadow-sm transition-all active:scale-95 font-medium"
                                    >
                                        <Dices size={16} /> {t('avatar.randomize', '🎲 换一个')}
                                    </button>
                                </div>
                            </div>

                            <div className="bg-white dark:bg-[#333333] border border-gray-100 dark:border-white/5 rounded-xl p-4 shadow-sm space-y-4">
                                <div>
                                    <h4 className="text-xs font-semibold text-gray-400 uppercase tracking-wide mb-3">{t('avatar.style', '绘画风格')}</h4>
                                    <div className="grid grid-cols-3 gap-2">
                                        {AVATAR_STYLES.map((style, idx) => (
                                            <button
                                                key={style.id}
                                                onClick={() => setGenStyleIdx(idx)}
                                                className={`py-2 px-2 text-[12px] rounded-lg border text-center transition-all ${genStyleIdx === idx
                                                    ? 'border-[#07c160] bg-[#07c160]/10 text-[#07c160] font-medium'
                                                    : 'border-gray-200 dark:border-white/10 text-gray-600 dark:text-gray-300 hover:border-gray-300 dark:hover:border-white/20'}`}
                                            >
                                                {style.title}
                                            </button>
                                        ))}
                                    </div>
                                </div>

                                {AVATAR_STYLES[genStyleIdx].bgAllowed && (
                                    <div className="pt-2 border-t border-dashed border-gray-100 dark:border-white/10">
                                        <h4 className="text-xs font-semibold text-gray-400 uppercase tracking-wide mb-3">{t('avatar.bg_color', '背景颜色')}</h4>
                                        <div className="flex flex-wrap gap-2">
                                            {BG_COLORS.map((bg, idx) => (
                                                <button
                                                    key={bg.id}
                                                    onClick={() => setGenBgIdx(idx)}
                                                    className={`w-7 h-7 rounded-full border-2 transition-all ${genBgIdx === idx ? 'border-[#07c160] scale-110 shadow-sm' : 'border-transparent hover:scale-105'} flex items-center justify-center`}
                                                    style={{ backgroundColor: bg.value === 'transparent' ? 'transparent' : `#${bg.value}` }}
                                                    title={bg.label}
                                                >
                                                    {bg.value === 'transparent' && <div className="w-full h-full rounded-full border border-gray-300 dark:border-gray-600 bg-gray-100 dark:bg-gray-800 flex items-center justify-center"><X size={12} className="text-gray-400" /></div>}
                                                    {genBgIdx === idx && bg.value !== 'transparent' && <Check size={12} className="text-black/50" />}
                                                </button>
                                            ))}
                                        </div>
                                    </div>
                                )}

                                <div className="pt-2 border-t border-dashed border-gray-100 dark:border-white/10">
                                    <div className="flex items-center justify-between">
                                        <h4 className="text-xs font-semibold text-gray-400 uppercase tracking-wide">{t('avatar.seed', '特征基因 (Seed)')}</h4>
                                        <input
                                            type="text"
                                            value={genSeed}
                                            onChange={(e) => setGenSeed(e.target.value)}
                                            className="w-1/2 bg-gray-50 dark:bg-black/20 border border-gray-200 dark:border-white/10 rounded-md px-2 py-1 text-xs text-gray-600 dark:text-gray-300 outline-none focus:border-[#07c160]"
                                            maxLength={20}
                                        />
                                    </div>
                                    <p className="text-[10px] text-gray-400 mt-2">修改基因字符可以微调当前的头像细节。</p>
                                </div>
                            </div>
                        </div>
                    )}

                    {/* Upload / Built-in Tab */}
                    {activeTab === 'upload' && (
                        <div className="animate-in fade-in duration-200">
                            <div className="flex items-center gap-4 mb-6 bg-white dark:bg-[#333333] border border-gray-100 dark:border-white/5 p-4 rounded-xl shadow-sm">
                                <div className="relative shrink-0">
                                    <div className="w-16 h-16 rounded-xl overflow-hidden bg-gray-100 dark:bg-black/20 flex items-center justify-center border border-gray-200 dark:border-white/10">
                                        {uploadPreviewUrl ? (
                                            <img src={uploadPreviewUrl} alt="Preview" className="w-full h-full object-cover" />
                                        ) : (
                                            <ImageIcon size={20} className="text-gray-300" />
                                        )}
                                    </div>
                                </div>
                                <div className="flex-1">
                                    <h4 className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                                        {t('avatar.upload_custom', '使用本地相册图片')}
                                    </h4>
                                    <input
                                        type="file"
                                        ref={fileInputRef}
                                        onChange={handleFileChange}
                                        accept="image/png, image/jpeg, image/webp"
                                        className="hidden"
                                    />
                                    <button
                                        onClick={() => fileInputRef.current?.click()}
                                        className="flex items-center gap-2 px-3 py-1.5 text-sm bg-gray-100 dark:bg-white/5 hover:bg-gray-200 dark:hover:bg-white/10 text-gray-700 dark:text-gray-300 rounded-lg transition-colors"
                                    >
                                        <Upload size={14} />
                                        {t('avatar.choose_file', '选择图片...')}
                                    </button>
                                </div>
                            </div>

                            <div>
                                <h4 className="text-sm font-medium text-gray-500 mb-3 ml-1">
                                    {t('avatar.built_in_logo', '系统应用图标')}
                                </h4>
                                <div className="grid grid-cols-4 gap-3">
                                    {BUILT_IN_AVATARS.map(avatar => (
                                        <div
                                            key={avatar.id}
                                            onClick={() => {
                                                setUploadPreviewUrl(avatar.url);
                                            }}
                                            className={`relative aspect-square rounded-xl overflow-hidden cursor-pointer border-2 transition-all group bg-white dark:bg-[#333333] shadow-sm ${uploadPreviewUrl === avatar.url ? 'border-[#07c160]' : 'border-transparent hover:border-gray-300 dark:hover:border-gray-500'}`}
                                        >
                                            <div className="w-full h-full flex items-center justify-center p-2">
                                                <img src={avatar.url} alt={avatar.title} className="w-full h-full object-contain drop-shadow-sm" />
                                            </div>
                                            {uploadPreviewUrl === avatar.url && (
                                                <div className="absolute top-1 right-1 w-4 h-4 bg-[#07c160] rounded-full flex items-center justify-center text-white shadow-sm">
                                                    <Check size={10} strokeWidth={3} />
                                                </div>
                                            )}
                                        </div>
                                    ))}
                                </div>
                            </div>
                        </div>
                    )}
                </div>

                <div className="px-5 py-4 border-t border-gray-100 dark:border-white/5 flex justify-between items-center bg-gray-50 dark:bg-[#2c2c2c]">
                    <div className="text-[11px] text-gray-400 flex items-center gap-1.5">
                        {activeTab === 'generator' && <span>⚡ Avatars powered by DiceBear</span>}
                    </div>
                    <div className="flex gap-2">
                        <button
                            onClick={onClose}
                            className="px-5 py-2 text-sm font-medium text-gray-600 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-white/10 rounded-xl transition-colors"
                        >
                            {t('common.cancel', '取消')}
                        </button>
                        <button
                            onClick={handleConfirm}
                            className={`px-5 py-2 text-sm font-medium text-white rounded-xl shadow-sm transition-all flex items-center gap-2 ${finalUrl ? 'bg-[#07c160] hover:bg-[#06ad56] hover:shadow-md' : 'bg-gray-300 dark:bg-gray-700 cursor-not-allowed'}`}
                            disabled={!finalUrl}
                        >
                            <Check size={16} /> {t('common.confirm', '确认使用')}
                        </button>
                    </div>
                </div>
            </div>
        </div>
    );
}
