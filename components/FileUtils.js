.pragma library

function basename(path) {
    if (!path) return "";
    var idx = path.lastIndexOf("/");
    return idx >= 0 ? path.slice(idx + 1) : path;
}

function dirname(path) {
    if (!path || path === "/") return "/";
    var p = path.endsWith("/") && path.length > 1 ? path.slice(0, -1) : path;
    var idx = p.lastIndexOf("/");
    return idx <= 0 ? "/" : p.slice(0, idx);
}

function joinPath(dir, name) {
    if (!dir || dir === "/") return "/" + name;
    return dir.endsWith("/") ? (dir + name) : (dir + "/" + name);
}

function isImageFile(name) {
    return /\.(png|jpe?g|webp|bmp|gif|svg|avif|ico)$/i.test(name || "");
}

function isTextFile(name) {
    return /\.(txt|md|json|yaml|yml|toml|conf|ini|sh|bash|zsh|py|qml|js|ts|jsx|tsx|rs|c|cpp|h|hpp|css|scss|html|xml|log|service|desktop|list)$/i.test(name || "");
}

function fileIcon(name) {
    var n = (name || "").toLowerCase();
    if (isImageFile(n)) return "󰸉";
    if (/\.(mp4|mkv|webm|avi|mov|flv|ts)$/.test(n)) return "󰕧";
    if (/\.(mp3|flac|wav|ogg|m4a|opus)$/.test(n)) return "󰎆";
    if (/\.pdf$/.test(n)) return "󰈦";
    if (/\.(zip|tar|gz|xz|zst|7z|rar)$/.test(n)) return "󰗄";
    if (isTextFile(n)) return "󰈙";
    return "󰈔";
}

function formatSize(bytes) {
    if (bytes === undefined || bytes === null || bytes < 0) return "";
    if (bytes < 1024) return bytes + " B";
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + " KB";
    if (bytes < 1024 * 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(1) + " MB";
    return (bytes / (1024 * 1024 * 1024)).toFixed(2) + " GB";
}
