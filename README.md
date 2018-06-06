## Race

Toy process tracer written in Rust. WIP

Example trace output:
```
$ race mate-session
mate-session 
\_ mate-session 
\_ /usr/lib/mate-settings-daemon/mate-settings-daemon 
| \_ xrdb -merge -quiet 
| | \_ /usr/bin/cpp -P -DHOST=ds3
| |   \_ /usr/lib/gcc/x86_64-pc-linux-gnu/8.1.0/cc1 -E -quiet -P -D HOST=ds3
| \_ /usr/bin/xkbcomp -w0 -I -I/usr/share/X11/xkb -xkm /tmp/fileP3GiKb /tmp/filefuRKPQ 
| \_ /usr/lib/mate-settings-daemon/mate-settings-daemon 
| | \_ rofi -show run 
| \_ /usr/lib/mate-settings-daemon/mate-settings-daemon 
|   \_ urxvt 
|     \_ zsh 
|       \_ zsh -f /home/max/.oh-my-zsh/tools/check_for_upgrade.sh 
|       | \_ mkdir /home/max/.oh-my-zsh/log/update.lock 
|       \_ zsh 
|       | \_ git --version 
|       \_ zsh 
\_ marco 
\_ mate-panel 
\_ caja 
| \_ net usershare info 
\_ mate-maximus 
\_ mate-volume-control-applet 
\_ mate-screensaver 
| \_ mate-screensaver 
\_ mate-power-manager 
| \_ /usr/bin/mate-power-backlight-helper --get-max-brightness 
| \_ mate-power-manager 
\_ /usr/lib/mate-polkit/polkit-mate-authentication-agent-1 
\_ /bin/sh /usr/bin/start-pulseaudio-x11 
  \_ /usr/bin/pactl load-module module-x11-publish display=:0 
  \_ /usr/bin/pactl load-module module-x11-xsmp display=:0 session_manager=local/ds3:@/tmp/.ICE-unix/11945,unix/ds3:/tmp/.ICE-unix/11945 
```
