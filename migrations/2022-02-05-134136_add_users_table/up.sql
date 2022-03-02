-- Your SQL goes here
create table users
(
    id       varchar not null
        constraint users_pk
            primary key,
    username varchar not null
);

create unique index users_id_uindex
    on users (id);

create unique index users_username_uindex
    on users (username);

