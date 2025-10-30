import { useCallback, useEffect, useState, type KeyboardEvent } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

type PermissionState = 'checking' | 'granted' | 'denied';

type FileAssociation = {
  extension: string;
  applicationName: string;
  applicationPath: string;
};

export default function App() {
  const [permission, setPermission] = useState<PermissionState>('checking');
  const [associations, setAssociations] = useState<FileAssociation[]>([]);
  const [loading, setLoading] = useState(false);
  const [feedback, setFeedback] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [newExtension, setNewExtension] = useState('');
  const [query, setQuery] = useState('');
  const [showTop, setShowTop] = useState(false);

  // Popular formats order for sorting (lower rank appears first)
  const popularOrder = [
    // Office / docs
    'doc', 'docx', 'xls', 'xlsx', 'ppt', 'pptx',
    'pdf', 'txt', 'md', 'markdown',
    // images
    'png', 'jpg', 'jpeg', 'gif',
    // videos
    'mp4', 'mov',
    // web
    'html', 'css', 'js', 'ts',
    // data
    'csv', 'json', 'xml',
    // archives (placed after docs/images/videos/web/data)
    'zip', 'rar', '7z', 'tar', 'gz',
  ];
  const rank = new Map(popularOrder.map((ext, i) => [ext, i]));

  const checkPermission = useCallback(async () => {
    setPermission('checking');
    try {
      const granted = await invoke<boolean>('check_full_disk_access');
      setPermission(granted ? 'granted' : 'denied');
      if (!granted) {
        setFeedback(null);
      }
      return granted;
    } catch (err) {
      console.error(err);
      setError('无法检测磁盘访问权限，请稍后再试。');
      setPermission('denied');
      return false;
    }
  }, []);

  const fetchAssociations = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await invoke<FileAssociation[]>('list_file_associations');
      // sort: by popularity rank first, then alphabetically
      const sorted = [...data].sort((a, b) => {
        const ra = rank.get(a.extension.toLowerCase()) ?? Number.MAX_SAFE_INTEGER;
        const rb = rank.get(b.extension.toLowerCase()) ?? Number.MAX_SAFE_INTEGER;
        if (ra !== rb) return ra - rb;
        return a.extension.localeCompare(b.extension);
      });
      setAssociations(sorted);
    } catch (err) {
      console.error(err);
      setError('读取默认应用列表失败，请刷新或稍后再试。');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    (async () => {
      const granted = await checkPermission();
      if (granted) {
        fetchAssociations();
      }
    })();
  }, [checkPermission, fetchAssociations]);

  const handleOpenSettings = useCallback(async () => {
    setError(null);
    setFeedback(null);
    try {
      await invoke('open_full_disk_access_settings');
      setFeedback('已打开系统偏好设置，请在“完全磁盘访问”中开启权限。');
    } catch (err) {
      console.error(err);
      setError('无法打开系统偏好设置，请手动前往"系统设置 > 隐私与安全 > 完全磁盘访问"。');
    }
  }, []);

  const handleRefreshPermission = useCallback(async () => {
    setFeedback(null);
    const granted = await checkPermission();
    if (granted) {
      fetchAssociations();
    }
  }, [checkPermission, fetchAssociations]);

  const handleModify = useCallback(
    async (extension: string) => {
      setError(null);
      try {
        const selection = await open({
          defaultPath: '/Applications',
          multiple: false,
          directory: false,
          canCreateDirectories: false,
          filters: [
            {
              name: '应用程序',
              extensions: ['app'],
            },
          ],
        });

        if (!selection || Array.isArray(selection)) {
          return;
        }

        await invoke('set_default_application_for_extension', {
          extension,
          applicationPath: selection,
        });
        setFeedback(`已更新 .${extension} 的默认打开方式。`);
        fetchAssociations();
      } catch (err) {
        console.error(err);
        setFeedback(null);
        const message =
          typeof err === 'string'
            ? err
            : err instanceof Error
              ? err.message
              : JSON.stringify(err);
        setError(
          message && !message.includes('更新默认应用失败')
            ? `更新默认应用失败：${message}`
            : message || '更新默认应用失败，请确认选择的应用有效。',
        );
      }
    },
    [fetchAssociations],
  );

  const handleAddExtension = useCallback(async () => {
    setFeedback(null);
    setError(null);
    const normalized = newExtension.trim().replace(/^\.+/, '').toLowerCase();
    if (!normalized) {
      setError('请输入有效的扩展名。');
      return;
    }
    setLoading(true);
    try {
      const data = await invoke<FileAssociation[]>('add_extension', { extension: normalized });
      setAssociations(data);
      setFeedback(`已添加 .${normalized} 文件类型。`);
      setNewExtension('');

      // 立即打开应用选择对话框来设置默认应用
      try {
        const selection = await open({
          defaultPath: '/Applications',
          multiple: false,
          directory: false,
          canCreateDirectories: false,
          filters: [
            {
              name: '应用程序',
              extensions: ['app'],
            },
          ],
        });

        if (selection && !Array.isArray(selection)) {
          await invoke('set_default_application_for_extension', {
            extension: normalized,
            applicationPath: selection,
          });
          setFeedback(`已为 .${normalized} 设置默认打开方式。`);
          await fetchAssociations();
        }
      } catch (err) {
        console.error(err);
        const message =
          typeof err === 'string'
            ? err
            : err instanceof Error
              ? err.message
              : JSON.stringify(err);
        if (message && !message.includes('用户取消了选择')) {
          setError(`设置默认应用失败：${message}`);
        }
      }
    } catch (err) {
      console.error(err);
      const message =
        typeof err === 'string' ? err : err instanceof Error ? err.message : JSON.stringify(err);
      setError(message || '添加文件类型失败，请稍后再试。');
    } finally {
      setLoading(false);
    }
  }, [newExtension, fetchAssociations]);

  const handleAddExtensionKey = useCallback(
    (event: KeyboardEvent<HTMLInputElement>) => {
      if (event.key === 'Enter') {
        event.preventDefault();
        handleAddExtension();
      }
    },
    [handleAddExtension],
  );

  const renderPermissionGate = () => (
    <div className="permission-card">
      <div>
        <h2>需要完全磁盘访问权限</h2>
        <p>
          应用需要读取系统的文件关联信息，请在 macOS 系统设置中开启“完全磁盘访问”权限。
        </p>
      </div>
      {error && <div className="refresh-banner" style={{ color: '#dc2626' }}>{error}</div>}
      {feedback && <div className="refresh-banner">{feedback}</div>}
      <div className="button-row">
        <button className="button button-primary" onClick={handleOpenSettings}>
          打开系统设置
        </button>
        <button className="button button-secondary" onClick={handleRefreshPermission}>
          我已授权，重新检测
        </button>
      </div>
    </div>
  );

  const renderAssociations = () => (
    <div className="list-card">
      <div className="list-toolbar">
        <input
          className="search-input"
          placeholder="搜索扩展名或应用名"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
      </div>
      <div className="list-header">
        <span>文件类型</span>
        <span>默认应用</span>
        <span></span>
      </div>
      {loading ? (
        <div className="empty-state">正在加载默认应用列表…</div>
      ) : associations.length ? (
        associations
          .filter((item) => {
            const q = query.trim().toLowerCase();
            if (!q) return true;
            return (
              item.extension.toLowerCase().includes(q) ||
              item.applicationName.toLowerCase().includes(q)
            );
          })
          .map((item) => (
          <div className="list-row" key={item.extension}>
            <span className="extension-pill">.{item.extension}</span>
            <div className="app-name">
              <span>{item.applicationName}</span>
              <span>{item.applicationPath}</span>
            </div>
            <button
              className="button button-secondary modify-button"
              onClick={() => handleModify(item.extension)}
            >
              修改默认应用
            </button>
          </div>
          ))
      ) : (
        <div className="empty-state">没有检测到文件类型，请稍后刷新。</div>
      )}
    </div>
  );

  return (
    <div className="app-shell">
      <div className="app-header">
       <div>
          <h1 className="app-title">默认打开方式管理器</h1>
          <p className="app-subtitle">为常见文件类型快速调整默认打开应用。</p>
        </div>
        {permission === 'granted' && <span className="status-indicator">权限已开启</span>}
      </div>

      {permission !== 'granted' ? (
        renderPermissionGate()
      ) : (
        <>
          {feedback && <div className="refresh-banner">{feedback}</div>}
          {error && <div className="refresh-banner" style={{ color: '#dc2626' }}>{error}</div>}
          <div className="refresh-banner" style={{ borderStyle: 'dashed' }}>
            <span>
              如果系统设置有变更，请点击按钮重新载入列表。
            </span>
            <div style={{ display: 'flex', gap: '0.5rem', flexWrap: 'wrap' }}>
              <button onClick={fetchAssociations} disabled={loading}>
                刷新列表
              </button>
              <div className="add-extension-form">
                <input
                  value={newExtension}
                  onChange={(event) => setNewExtension(event.target.value)}
                  onKeyDown={handleAddExtensionKey}
                  placeholder="添加扩展名并设置默认应用 (例如 md)"
                  disabled={loading}
                />
                <button onClick={handleAddExtension} disabled={loading}>
                  添加文件类型
                </button>
              </div>
            </div>
          </div>
          {renderAssociations()}
          <button
            className="scroll-top"
            onClick={() => window.scrollTo({ top: 0, behavior: 'smooth' })}
            style={{ opacity: showTop ? 1 : 0, pointerEvents: showTop ? 'auto' : 'none' }}
            aria-label="回到顶部"
            title="回到顶部"
          >
            ↑ 回到顶部
          </button>
        </>
      )}
    </div>
  );
}
