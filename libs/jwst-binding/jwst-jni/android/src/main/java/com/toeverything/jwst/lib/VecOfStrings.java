// Automatically generated by flapigen
package com.toeverything.jwst.lib;
import androidx.annotation.NonNull;

public final class VecOfStrings {

    public VecOfStrings() {
        mNativeObj = init();
    }
    private static native long init();

    public final @NonNull String at(long i) {
        String ret = do_at(mNativeObj, i);

        return ret;
    }
    private static native @NonNull String do_at(long self, long i);

    public final long len() {
        long ret = do_len(mNativeObj);

        return ret;
    }
    private static native long do_len(long self);

    public final void push(@NonNull String s) {
        do_push(mNativeObj, s);
    }
    private static native void do_push(long self, @NonNull String s);

    public final void insert(long i, @NonNull String s) {
        do_insert(mNativeObj, i, s);
    }
    private static native void do_insert(long self, long i, @NonNull String s);

    public final void clear() {
        do_clear(mNativeObj);
    }
    private static native void do_clear(long self);

    public final void remove(long i) {
        do_remove(mNativeObj, i);
    }
    private static native void do_remove(long self, long i);

    public final void remove_item(@NonNull String s) {
        do_remove_item(mNativeObj, s);
    }
    private static native void do_remove_item(long self, @NonNull String s);

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
    /*package*/ VecOfStrings(InternalPointerMarker marker, long ptr) {
        assert marker == InternalPointerMarker.RAW_PTR;
        this.mNativeObj = ptr;
    }
    /*package*/ long mNativeObj;
}