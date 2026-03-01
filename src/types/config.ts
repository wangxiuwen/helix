export interface AppConfig {
    language: string;
    theme: string;
    appAvatarUrl?: string;
    hidden_menu_items?: string[];
    ai_config?: {
        provider: string;
        base_url: string;
        api_key: string;
        model: string;
        max_tokens: number;
        system_prompt: string;
        auto_reply: boolean;
    };
}
