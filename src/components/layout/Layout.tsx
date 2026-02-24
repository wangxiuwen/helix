import { Outlet } from 'react-router-dom';
import { getCurrentWindow } from '@tauri-apps/api/window';
import Navbar from '../navbar/Navbar';
import ToastContainer from '../common/ToastContainer';
import { isTauri } from '../../utils/env';

function Layout() {
    return (
        <div className="h-screen flex flex-col bg-[#FAFBFC] dark:bg-base-300">
            {/* Window drag area - Tauri only */}
            {isTauri() && (
                <div
                    className="fixed top-0 left-0 right-0 h-9"
                    style={{
                        zIndex: 9999,
                        backgroundColor: 'rgba(0,0,0,0.001)',
                        cursor: 'default',
                        userSelect: 'none',
                        WebkitUserSelect: 'none'
                    }}
                    data-tauri-drag-region
                    onMouseDown={() => {
                        getCurrentWindow().startDragging();
                    }}
                />
            )}
            <ToastContainer />
            <Navbar />
            <main className="flex-1 overflow-hidden flex flex-col relative">
                <Outlet />
            </main>
        </div>
    );
}

export default Layout;
