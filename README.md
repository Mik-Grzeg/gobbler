# gobbler
Wallpaper changer for standalone window managers based on X11

It watches a directory where you store your wallpapers. Wallpapers are chosen from that directory and the set using `feh`. 
Interval for changing wallpapers can be configures with flag. 

User can also control running daemon by calling that program with specific agruments to change manually to the next or previous wallpaper in the queue.

## Requirements:
* [feh](https://feh.finalrewind.org/)

## Modes
* daemon:
  * changing wallpapers
  * watching for new wallpapers in provided directory by the user
  * listens to client events which can be triggered by the user
* client:
  * can be used to change to next or previous wallpaper
 
## How to

### Start daemon
Personally I put it in the startup configuration of the window manager

It can be started as such:
```
gobbler start -d <directory-of-the-wallpapers>
```

Default configuration can be easily changed with flags, more details can be found with
`gobbler start -h`

### Use client to change wallpaper to the next
While doing that the daemon has to be running
```
gobbler do next
```
