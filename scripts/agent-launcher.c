// Bench Agent.app's program: the parent of this Mac's agent, so the agent does not hang on
// Bench. macOS judges a process's local-network access by the app it runs under; started by
// Bench, the agent ran under Bench, and every rebuild of Bench (ship.sh quits it) left it under
// a dead app. Under this small app, which nothing quits and which rarely changes (its grant is
// kept by its signature and Mach-O UUID), it lives on through Bench's restarts.
//
// Started by Bench with `open -g -n "Bench Agent.app" --env … --args <log> <folder> <program> …`:
// runs the program in the folder with its output in the log, and exits when it does.

#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <spawn.h>
#include <stdio.h>
#include <sys/wait.h>
#include <unistd.h>

extern char **environ;

static pid_t child;

static void pass(int sig) {
    if (child > 0) kill(child, sig);
}

int main(int argc, char **argv) {
    if (argc < 4) {
        fprintf(stderr, "usage: %s <log> <folder> <program> [args…]\n", argv[0]);
        return 2;
    }
    if (chdir(argv[2]) != 0) return 3;
    int log = open(argv[1], O_WRONLY | O_CREAT | O_TRUNC, 0644);
    posix_spawn_file_actions_t io;
    posix_spawn_file_actions_init(&io);
    posix_spawn_file_actions_addopen(&io, 0, "/dev/null", O_RDONLY, 0);
    if (log >= 0) {
        posix_spawn_file_actions_adddup2(&io, log, 1);
        posix_spawn_file_actions_adddup2(&io, log, 2);
    }
    if (posix_spawn(&child, argv[3], &io, NULL, argv + 3, environ) != 0) return 4;
    signal(SIGTERM, pass);
    signal(SIGINT, pass);
    int status = 0;
    while (waitpid(child, &status, 0) < 0)
        if (errno != EINTR) return 1;
    return WIFEXITED(status) ? WEXITSTATUS(status) : 1;
}
