package com.mybevy.project;

import android.content.res.Configuration;
import android.os.Bundle;
import android.view.View;
import android.view.WindowManager;

import androidx.core.graphics.Insets;
import androidx.core.view.WindowInsetsCompat;

import com.google.androidgamesdk.GameActivity;

public class MainActivity extends GameActivity {
    private int lastInsetLeft = -1;
    private int lastInsetRight = -1;
    private int lastInsetTop = -1;
    private int lastInsetBottom = -1;

    private static native void nativeOnWindowInsetsChanged(
        int left,
        int right,
        int top,
        int bottom
    );

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        getWindow().setDecorFitsSystemWindows(false);
        WindowManager.LayoutParams attributes = getWindow().getAttributes();
        attributes.layoutInDisplayCutoutMode =
            WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES;
        getWindow().setAttributes(attributes);
        getWindow().getDecorView().requestApplyInsets();
    }

    @Override
    protected void onResume() {
        super.onResume();
        getWindow().getDecorView().post(
            () -> getWindow().getDecorView().requestApplyInsets()
        );
    }

    @Override
    public WindowInsetsCompat onApplyWindowInsets(View view, WindowInsetsCompat insets) {
        WindowInsetsCompat applied = super.onApplyWindowInsets(view, insets);
        int persistentTypes = WindowInsetsCompat.Type.systemBars()
            | WindowInsetsCompat.Type.displayCutout()
            | WindowInsetsCompat.Type.mandatorySystemGestures();
        Insets persistent = insets.getInsetsIgnoringVisibility(persistentTypes);
        Insets gestures = insets.getInsets(WindowInsetsCompat.Type.systemGestures());
        publishInsets(
            Math.max(persistent.left, gestures.left),
            Math.max(persistent.right, gestures.right),
            Math.max(persistent.top, gestures.top),
            Math.max(persistent.bottom, gestures.bottom)
        );
        return applied;
    }

    @Override
    public void onConfigurationChanged(Configuration newConfig) {
        super.onConfigurationChanged(newConfig);
        getWindow().getDecorView().post(
            () -> getWindow().getDecorView().requestApplyInsets()
        );
    }

    @Override
    public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);

        if (hasFocus) {
            hideSystemUi();
            getWindow().getDecorView().requestApplyInsets();
        }
    }

    private void publishInsets(int left, int right, int top, int bottom) {
        if (left == lastInsetLeft
            && right == lastInsetRight
            && top == lastInsetTop
            && bottom == lastInsetBottom) {
            return;
        }
        lastInsetLeft = left;
        lastInsetRight = right;
        lastInsetTop = top;
        lastInsetBottom = bottom;
        nativeOnWindowInsetsChanged(left, right, top, bottom);
    }

    private void hideSystemUi() {
        View decorView = getWindow().getDecorView();
        decorView.setSystemUiVisibility(
            View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
                | View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
                | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                | View.SYSTEM_UI_FLAG_FULLSCREEN
        );
    }
}
