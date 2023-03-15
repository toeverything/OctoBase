// Automatically generated by flapigen
package com.toeverything.jwst.lib;
import androidx.annotation.NonNull;

public final class Workspace {

    public Workspace(@NonNull String _id) {
        mNativeObj = init(_id);
    }
    private static native long init(@NonNull String _id);

    public final @NonNull String id() {
        String ret = do_id(mNativeObj);

        return ret;
    }
    private static native @NonNull String do_id(long self);

    public final long clientId() {
        long ret = do_clientId(mNativeObj);

        return ret;
    }
    private static native long do_clientId(long self);

    public final @NonNull java.util.Optional<Block> get(@NonNull WorkspaceTransaction trx, @NonNull String block_id) {
        long a0 = trx.mNativeObj;
        long ret = do_get(mNativeObj, a0, block_id);
        java.util.Optional<Block> convRet;
        if (ret != 0) {
            convRet = java.util.Optional.of(new Block(InternalPointerMarker.RAW_PTR, ret));
        } else {
            convRet = java.util.Optional.empty();
        }

        JNIReachabilityFence.reachabilityFence1(trx);

        return convRet;
    }
    private static native long do_get(long self, long trx, @NonNull String block_id);

    public final boolean exists(@NonNull WorkspaceTransaction trx, @NonNull String block_id) {
        long a0 = trx.mNativeObj;
        boolean ret = do_exists(mNativeObj, a0, block_id);

        JNIReachabilityFence.reachabilityFence1(trx);

        return ret;
    }
    private static native boolean do_exists(long self, long trx, @NonNull String block_id);

    public final boolean withTrx(@NonNull OnWorkspaceTransaction on_trx) {
        boolean ret = do_withTrx(mNativeObj, on_trx);

        return ret;
    }
    private static native boolean do_withTrx(long self, OnWorkspaceTransaction on_trx);

    public final void dropTrx(@NonNull WorkspaceTransaction trx) {
        long a0 = trx.mNativeObj;
        trx.mNativeObj = 0;

        do_dropTrx(mNativeObj, a0);

        JNIReachabilityFence.reachabilityFence1(trx);
    }
    private static native void do_dropTrx(long self, long trx);

    public final @NonNull String search(@NonNull String query) {
        String ret = do_search(mNativeObj, query);

        return ret;
    }
    private static native @NonNull String do_search(long self, @NonNull String query);

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
    /*package*/ Workspace(InternalPointerMarker marker, long ptr) {
        assert marker == InternalPointerMarker.RAW_PTR;
        this.mNativeObj = ptr;
    }
    /*package*/ long mNativeObj;
}