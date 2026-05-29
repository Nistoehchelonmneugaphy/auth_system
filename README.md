Basic idea:
1. When the main website has a new subscription (I am talking about the so-called manual_server_2 repo), it will call a webhook of this repo's server, adding the user's username and password hash into this DB, as well as subscriptino expiry date. Upon additional subscriptions, the same webhook is called, it just updates expiry instead of inserting the whole entry, I guess.
2. Later on, a user logs in the client. They enter password and username. The client sends a request to this server running somewhere. This server checks if data valid and not expired, etc. If yes, it creates a temp token, maybe? And returns it.
3. Client sends requests that requrie being authenticated, like "send global chat message", "send a new friend invite" or "join the matchmaking queue". It sends the token together with each request.
4. The lobby server sends a request to verify the token to this auth server on each request.


So, I suppose the lobby server will have the url of this auth server as an environment variable or some such.
The auth server will have the private API tokens which will allow the website app to add/remove/extend valid user subscriptions/etc.
