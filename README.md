# Simple Web Server
This is a simple HTTP/1.1 web server primarily for serving static pages.
However a Lua scripting backend does exist to accommodate more advanced use
cases.

Staying true to its name, the server is configured through one json file
that defines the port the server will run on, the file to serve for the home
page, the file to server for a 404 error, and any custom urls pointing to other
files on the system.

There are plans to allow the server to use either HTTP/1.1 or HTTP/2 in the
future. Adding HTTP/3 support to the server is currently not planned as the
underlying HTTP library [hyper](https://github.com/hyperium/hyper) currently does not have support for the
protocol.
