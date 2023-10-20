// Automatically generated by flapigen
package com.toeverything.jwst.lib;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;

public final class JwstStorage {

    public JwstStorage(@NonNull String path) {
        mNativeObj = init(path);
    }
    private static native long init(@NonNull String path);

    public JwstStorage(@NonNull String path, @NonNull String level) {
        mNativeObj = init(path, level);
    }
    private static native long init(@NonNull String path, @NonNull String level);

    public final @NonNull java.util.Optional<String> error() {
        String ret = do_error(mNativeObj);
        java.util.Optional<String> convRet = java.util.Optional.ofNullable(ret);

        return convRet;
    }
    private static native @Nullable String do_error(long self);

    public final boolean is_offline() {
        boolean ret = do_is_offline(mNativeObj);

        return ret;
    }
    private static native boolean do_is_offline(long self);

    public final boolean is_connected() {
        boolean ret = do_is_connected(mNativeObj);

        return ret;
    }
    private static native boolean do_is_connected(long self);

    public final boolean is_finished() {
        boolean ret = do_is_finished(mNativeObj);

        return ret;
    }
    private static native boolean do_is_finished(long self);

    public final boolean is_error() {
        boolean ret = do_is_error(mNativeObj);

        return ret;
    }
    private static native boolean do_is_error(long self);

    public final @NonNull String get_sync_state() {
        String ret = do_get_sync_state(mNativeObj);

        return ret;
    }
    private static native @NonNull String do_get_sync_state(long self);

    public final boolean import_workspace(@NonNull String workspace_id, @NonNull byte [] data) {
        boolean ret = do_import_workspace(mNativeObj, workspace_id, data);

        return ret;
    }
    private static native boolean do_import_workspace(long self, @NonNull String workspace_id, byte [] data);

    public final byte [] export_workspace(@NonNull String workspace_id) {
        byte [] ret = do_export_workspace(mNativeObj, workspace_id);

        return ret;
    }
    private static native byte [] do_export_workspace(long self, @NonNull String workspace_id);

    public final @NonNull java.util.Optional<Workspace> connect(@NonNull String workspace_id, @NonNull String remote) {
        long ret = do_connect(mNativeObj, workspace_id, remote);
        java.util.Optional<Workspace> convRet;
        if (ret != 0) {
            convRet = java.util.Optional.of(new Workspace(InternalPointerMarker.RAW_PTR, ret));
        } else {
            convRet = java.util.Optional.empty();
        }

        return convRet;
    }
    private static native long do_connect(long self, @NonNull String workspace_id, @NonNull String remote);

    public final long [] get_last_synced() {
        long [] ret = do_get_last_synced(mNativeObj);

        return ret;
    }
    private static native long [] do_get_last_synced(long self);

    public synchronized void delete() {
        if (mNativeObj != 0) {
            do_delete(mNativeObj);
            mNativeObj = 0;
       }
    }
    @Override
    protected void finalize() throws Throwable {
        try {
            delete();
        }
        finally {
             super.finalize();
        }
    }
    private static native void do_delete(long me);
    /*package*/ JwstStorage(InternalPointerMarker marker, long ptr) {
        assert marker == InternalPointerMarker.RAW_PTR;
        this.mNativeObj = ptr;
    }
    /*package*/ long mNativeObj;
}