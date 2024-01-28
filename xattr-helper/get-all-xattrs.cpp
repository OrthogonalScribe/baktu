/*
 * Baktu helper tool for reading the full set of extended attributes of files
 *
 * Reads \0-separated paths on stdin, prints the xattrs for each, including
 * those in the `trusted` namespace, which require CAP_SYS_ADMIN to be listed.
 *
 * This helper exists solely to reduce the amount of code given access to
 * CAP_SYS_ADMIN.
 */

#include <cstdlib>
#include <cstring>
#include <iostream>
#include <vector>

#include <sys/capability.h>
#include <sys/xattr.h>

void raise_cap_sys_admin() {
	cap_t caps;
	const cap_value_t cap_list[1] = { CAP_SYS_ADMIN };

	if (!CAP_IS_SUPPORTED(CAP_SETFCAP)) {
		std::cerr << "CAP_SETFCAP not supported" << std::endl;
		exit(EXIT_FAILURE);
	}

	caps = cap_get_proc();
	if (caps == NULL) {
		perror("cap_get_proc");
		cap_free(caps);
		exit(EXIT_FAILURE);
	}

	if (cap_set_flag(caps, CAP_EFFECTIVE, 2, cap_list, CAP_SET) == -1) {
		perror("cap_set_flag");
		cap_free(caps);
		exit(EXIT_FAILURE);
	}

	if (cap_set_proc(caps) == -1) {
		perror("cap_set_proc");
		std::cerr <<
			"  (`get-all-xattrs` needs CAP_SYS_ADMIN to show xattrs"
			" from all namespaces, re-run either after"
			" `setcap cap_sys_admin=p get-all-xattrs` or with `sudo`)"
			<< std::endl;
		cap_free(caps);
		exit(EXIT_FAILURE);
	}

	if (cap_free(caps) == -1) {
		perror("cap_free");
		exit(EXIT_FAILURE);
	}

	cap_free(caps);
}

void dump_xattrs(const char *filepath)
{
	ssize_t buflen, keylen;
	char *buf, *key;

	// Determine the length of the buffer needed.
	buflen = llistxattr(filepath, NULL, 0);
	if (buflen == -1) {
		perror("llistxattr");
		exit(EXIT_FAILURE);
	}

	if (buflen == 0)
		return; // file has no attributes

	buf = (char *) malloc(buflen);
	if (buf == NULL) {
		perror("malloc");
		exit(EXIT_FAILURE);
	}

	// Copy the list of attribute keys to the buffer.
	// TODO: (S) Fix the TOCTOU bug if the keys are changed between the two
	//   calls of llistxattr, see allocate_loop() in the xattr crate.
	buflen = llistxattr(filepath, buf, buflen);
	if (buflen == -1) {
		perror("llistxattr");
		exit(EXIT_FAILURE);
	}

	// Loop over the list of zero terminated strings with the
	// attribute keys. Use the remaining buffer length to determine
	// the end of the list.
	key = buf;
	while (buflen > 0) {
		static const char* hex_letter = "0123456789abcdef";
		for (const char* byteptr = key; *byteptr; ++byteptr) {
			putchar(hex_letter[(*byteptr & 0xF0) >> 4]);
			putchar(hex_letter[(*byteptr & 0x0F)     ]);
		}

		// Get and print value.
		// TODO: (S) Fix the equivalent TOCTOU bug for value fetching
		std::vector<char> val;

		ssize_t val_size = lgetxattr(filepath, key, NULL, 0);
		if (val_size == -1) {
			perror("lgetxattr");
			exit(EXIT_FAILURE);
		} else if (val_size == 0) {
			val.clear();
		} else if (val_size > 0) {
			val.resize(val_size);
			int rc = lgetxattr(filepath, key, &val[0], val.size());
			if (rc == -1) {
				perror("lgetxattr");
				exit(EXIT_FAILURE);
			}

			putchar(' ');
			for (const char& byte: val) {
				putchar(hex_letter[(byte & 0xF0) >> 4]);
				putchar(hex_letter[(byte & 0x0F)     ]);
			}
			putchar('\n'); // assume stdout flushes after a newline
		}

		// Go to next attribute key.
		keylen = strlen(key) + 1;
		buflen -= keylen;
		key += keylen;
	}

	free(buf);
}

int main() {
	raise_cap_sys_admin();

	for (std::string path; std::getline(std::cin, path, '\0'); ) {
		dump_xattrs(path.c_str());
		std::cout << "--" << std::endl;
	}

	return 0;
}
